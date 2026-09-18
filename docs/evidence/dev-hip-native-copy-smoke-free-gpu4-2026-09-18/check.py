#!/usr/bin/env python3
"""Strict complete observer envelope and predeclared fixed-window smoke audit."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import sys

from protocol import (
    BDF,
    BINARY_SHA,
    BUSY_ONLY,
    CHECKS,
    COMMIT,
    DELAYED_WINDOW_NS,
    IMMEDIATE_WINDOW_NS,
    NATIVE,
    NATIVE_ENV,
    OBSERVER,
    OBSERVER_SHA,
    OWNED,
    PAYLOAD_SHA,
    SOURCE,
    TOPOLOGY,
    UID,
    VISIBILITY,
)

HERE = Path(__file__).resolve().parent


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def unique(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def loads(text):
    return json.loads(text, object_pairs_hook=unique)


def imported(name, path, expected):
    need(digest(path) == expected, "pinned helper identity: " + str(path))
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


observer = imported(
    "smoke_observer",
    HERE / "source/benchmarks/runtime_gfx942/copy-host-observe.py",
    OBSERVER_SHA,
)
hip = imported("smoke_hip_payload", HERE / "hip_copy_diagnostic.py", PAYLOAD_SHA)
EXPECTED_HIP = hip.Expected(0, int(UID, 0), "gfx942:sramecc+:xnack-", 268435456, 3, 10)


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


def endpoint(text, receipt):
    need(
        receipt["error"] is None and receipt["group_absent"] is True,
        "observer command ownership",
    )
    lines = text.splitlines()
    need(len(lines) == 2 and text.endswith("\n"), "complete observer envelope")
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
        (
            row["schema"],
            row["record"],
            row["gpu_index"],
            row["pci_bdf"],
            row["unique_id"],
            row["index"],
        )
        == (observer.SCHEMA, "observation", 4, BDF, UID, 0),
        "observation identity",
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
    ordered(row["finished"], receipt["reaped"])
    need(len(row["sysfs"]) == 3, "three complete sysfs observations")
    previous = row["started"]
    for span in [
        row["sysfs"][0],
        row["status"],
        row["sysfs"][1],
        row["pids"],
        row["sysfs"][2],
    ]:
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
        need(snap["path"] == "/sys/bus/pci/devices/" + BDF, "sysfs BDF binding")
        try:
            values = snap["values"]
            need(
                not snap["errors"] and values["unique_id"].lower() == UID[2:],
                "sysfs identity",
            )
            need(set(values) == set(observer.METRICS), "sysfs metric roster")
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
            "CLI fields",
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
            "CLI command",
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
        "derived refusal and PID list",
    )
    need(
        row["endpoint_admitted"] is (not reasons),
        "original observer admission preserved",
    )
    need(
        end
        == {
            "schema": observer.SCHEMA,
            "record": "complete",
            "observations": 1,
            "refused": int(bool(reasons)),
            "all_endpoints_admitted": not reasons,
            "performance_accepted": False,
        },
        "observer terminal record",
    )
    need(
        type(receipt["exit"]) is int and receipt["exit"] == int(bool(reasons)),
        "original observer exit preserved",
    )
    return {
        "started": row["started"],
        "finished": row["finished"],
        "reasons": reasons,
        "selected_pids": selected,
        "original_admitted": not reasons,
    }


def phase_accepts(phase, observed, t0=None):
    need(phase in ("pre", "immediate", "delayed"), "unknown observation phase")
    if phase == "pre":
        return observed["original_admitted"] and observed["selected_pids"] == []
    delta = clock(observed["started"]) - clock(t0)
    low, high = IMMEDIATE_WINDOW_NS if phase == "immediate" else DELAYED_WINDOW_NS
    allowed = (
        set(observed["reasons"]) <= BUSY_ONLY
        if phase == "immediate"
        else not observed["reasons"]
    )
    return low <= delta <= high and allowed and observed["selected_pids"] == []


def passed(row):
    return row["exit"] == 0 and row["error"] is None and row["group_absent"] is True


def manifest(path):
    result = {}
    for line in path.read_text().splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  (.+)", line)
        need(match is not None and match[2] not in result, "unique canonical manifest")
        result[match[2]] = match[1]
    need(bool(result), "nonempty manifest")
    return result


def audit(results):
    root = results / "smoke"
    state = loads((results / "smoke.json").read_text())
    need(
        set(state)
        == {
            "schema",
            "source_commit",
            "started",
            "native_launched",
            "native_reaped",
            "observations",
            "failures",
            "performance_accepted",
            "smoke_accepted",
            "records",
            "finished",
        },
        "state fields",
    )
    need(
        state["schema"] == "fe2o3.hip-smoke-fixed-deadline.v1"
        and state["source_commit"] == COMMIT,
        "state source",
    )
    need(state["performance_accepted"] is False, "no performance acceptance")
    names, records = [], {}
    previous = state["started"]
    for index, path in enumerate(sorted(root.glob("*.json"))):
        row = loads(path.read_text())
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
                "reaped",
                "pid",
                "exit",
                "error",
                "group_absent",
                "outer_bound_seconds",
                "finished",
                "stdout_sha256",
                "stderr_sha256",
            },
            "command fields",
        )
        name = path.stem
        need(
            name.startswith(f"{index:03d}-") and row["name"] == name,
            "record order/name",
        )
        need(row["schema"] == "fe2o3.hip-smoke-command.v1", "command schema")
        need(
            row["cwd"] == str(SOURCE) and row["visibility_unset"] == VISIBILITY,
            "command environment/cwd",
        )
        ordered(previous, row["started"])
        ordered(row["started"], row["reaped"])
        ordered(row["reaped"], row["finished"])
        previous = row["finished"]
        need(
            type(row["pid"]) is int and row["pid"] > 0 and row["group_absent"] is True,
            "owned PID/group closure",
        )
        need(
            digest(root / (name + ".stdout")) == row["stdout_sha256"]
            and digest(root / (name + ".stderr")) == row["stderr_sha256"],
            "raw output identity",
        )
        names.append(name)
        records[name.split("-", 1)[1]] = row
    need(state["records"] == names, "complete state record roster")
    need(len(records) == len(names), "unique command tags")
    need(
        {p.name for p in root.iterdir()}
        == {
            name + suffix
            for name in names
            for suffix in (".json", ".stdout", ".stderr")
        },
        "closed record file set",
    )
    ordered(previous, state["finished"])
    expected = [tag + "-before" for tag, _ in CHECKS] + ["topology", "pre"]
    if state["native_launched"]:
        expected += ["native", "immediate", "delayed"]
    expected += [tag + "-after" for tag, _ in CHECKS]
    need(list(records) == expected, "exact normal/refused attempt roster")
    need(
        digest(HERE / "source-files.sha256")
        == "d796342d5c9e8b28ea759e65acc5de878a270db0268ea1239659384f3deeafcf",
        "source manifest pin",
    )
    need(
        digest(results / "platform.sha256")
        == "5911229d3fce10aceef828b65f4d5b21066d2fb173759bd2a178dc7acb024d2a",
        "platform manifest pin",
    )
    manifests = {
        "source": manifest(HERE / "source-files.sha256"),
        "binary": manifest(results / "binary.sha256"),
        "platform": manifest(results / "platform.sha256"),
        "scripts": manifest(HERE / "scripts.sha256"),
    }
    for name, expected_sha in manifests["source"].items():
        need(digest(HERE / "source" / name) == expected_sha, "archived source bytes")
    for name, expected_sha in manifests["scripts"].items():
        need(Path(name).parent == OWNED, "script path scope")
        need(
            digest(HERE / Path(name).name) == expected_sha,
            "current reviewed script bytes",
        )
    all_ok = True
    for tag, argv in CHECKS:
        for phase in ("before", "after"):
            row = records[f"{tag}-{phase}"]
            need(
                row["argv"] == argv and row["environment_override"] == {},
                "exact identity command",
            )
            need(
                (root / (row["name"] + ".stderr")).read_text() == "",
                "identity check stderr",
            )
            if passed(row):
                need(
                    (root / (row["name"] + ".stdout")).read_text()
                    == "".join(name + ": OK\n" for name in manifests[tag]),
                    "exact identity result roster",
                )
            all_ok &= passed(row)
    need(
        (results / "binary.sha256").read_text()
        == f"{BINARY_SHA}  {OWNED}/async-copy-hip\n",
        "prepared binary pin",
    )
    top = records["topology"]
    need(
        top["argv"] == TOPOLOGY and top["environment_override"] == {},
        "exact topology command",
    )
    if passed(top):
        need((root / (top["name"] + ".stderr")).read_text() == "", "topology stderr")
        topology = loads((root / (top["name"] + ".stdout")).read_text())
        expected_placement = [
            "policy: bind",
            "preferred node: 1",
            "physcpubind: " + " ".join(map(str, range(48, 96))),
            "cpubind: 1",
            "nodebind: 1",
            "membind: 1",
            "preferred: 1",
        ]
        need(
            set(topology)
            == {
                "unique_id",
                "numa_node",
                "local_cpulist",
                "affinity",
                "placement_stdout",
                "placement_stderr",
                "placement_exit",
            },
            "topology fields",
        )
        need(
            topology["unique_id"] == UID[2:]
            and topology["numa_node"] == "1"
            and topology["local_cpulist"] == "48-95",
            "selected topology",
        )
        need(
            topology["affinity"] == list(range(48, 96))
            and topology["placement_exit"] == 0
            and topology["placement_stderr"] == "",
            "effective placement",
        )
        need(
            [line.rstrip() for line in topology["placement_stdout"].splitlines()]
            == expected_placement,
            "NUMA binding",
        )
    all_ok &= passed(top)
    observations = {}
    for phase in ("pre", "immediate", "delayed"):
        if phase not in records:
            continue
        row = records[phase]
        need(
            row["argv"] == OBSERVER
            and row["environment_override"] == {}
            and row["outer_bound_seconds"] == 70,
            "exact observer command",
        )
        need((root / (row["name"] + ".stderr")).read_text() == "", "observer stderr")
        observations[phase] = endpoint(
            (root / (row["name"] + ".stdout")).read_text(), row
        )
    pre_ok = phase_accepts("pre", observations["pre"])
    accepted = False
    if state["native_launched"]:
        need(
            pre_ok
            and all(passed(records[f"{tag}-before"]) for tag, _ in CHECKS)
            and passed(top),
            "native launch admission",
        )
        row = records["native"]
        need(
            row["argv"] == NATIVE
            and row["environment_override"] == NATIVE_ENV
            and row["outer_bound_seconds"] == 200,
            "exact native command",
        )
        need(
            0
            <= clock(row["started"]) - clock(records["pre"]["finished"])
            <= 1_000_000_000,
            "fresh preflight launch",
        )
        need(state["native_reaped"] == row["reaped"], "T0 is native reaping timestamp")
        payload_ok = False
        try:
            hip.validate(
                (root / (row["name"] + ".stdout")).read_text(),
                (root / (row["name"] + ".stderr")).read_text(),
                row["exit"],
                EXPECTED_HIP,
            )
            payload_ok = passed(row)
        except ValueError:
            pass
        accepted = (
            all_ok
            and payload_ok
            and phase_accepts("immediate", observations["immediate"], row["reaped"])
            and phase_accepts("delayed", observations["delayed"], row["reaped"])
        )
    else:
        need(state["native_reaped"] is None, "no native timestamp on refusal")
        need(not pre_ok, "no unexplained successful preflight without launch")
    for phase, observed in observations.items():
        expected_phase = {
            **observed,
            "protocol_phase_accepted": bool(
                phase_accepts(phase, observed, state["native_reaped"])
            ),
        }
        need(
            state["observations"].get(phase) == expected_phase,
            "derived recorded observation classification",
        )
    need(set(state["observations"]) == set(observations), "exact observation roster")
    need(state["smoke_accepted"] is bool(accepted), "derived smoke outcome")
    need(bool(state["failures"]) is (not accepted), "sticky outcome diagnostics")
    return {
        "records": len(records),
        "native_launched": state["native_launched"],
        "smoke_accepted": bool(accepted),
        "observations": observations,
        "performance_accepted": False,
        "scope": "one-process prospective fixed-deadline protocol; not reservation or causal attribution",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("results", type=Path)
    parser.add_argument("--require-success", action="store_true")
    args = parser.parse_args()
    report = audit(args.results)
    print(json.dumps(report, indent=2))
    return int(args.require_success and not report["smoke_accepted"])


if __name__ == "__main__":
    raise SystemExit(main())
