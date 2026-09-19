#!/usr/bin/env python3
"""Offline identity, execution, observation and cleanup acceptance only."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run with python3 -I")

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
CAMPAIGN_SHA = "b8d6ab1e3188aa1eed03389e145a647acd8cda7a2f2a0d374b340363a0b7c0e2"


def authenticated_campaign(path):
    if (
        path.is_symlink()
        or not path.is_file()
        or hashlib.sha256(path.read_bytes()).hexdigest() != CAMPAIGN_SHA
    ):
        raise RuntimeError("campaign authenticated before importing packet code")
    spec = importlib.util.spec_from_file_location("xgmi_attribution_campaign", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


C = authenticated_campaign(HERE / "campaign.py")
need, read, sha = C.need, C.read, C.sha
LOCAL_ORDER = [
    "calibration",
    "observer-tests",
    "cpu-verify",
    "signature",
    "source",
    "source-ancestor",
    "source-clean",
    "create",
    "upload",
    "native",
    "remote-inventory",
    "collect",
    "cleanup",
    "absence",
]
RECEIPT_KEYS = {
    "command",
    "cwd",
    "started_ns",
    "finished_ns",
    "timeout_seconds",
    "pid",
    "exit",
    "error",
    "group_absent",
    "environment",
    "stdin_sha256",
    "stdout_sha256",
    "stderr_sha256",
}
PROTOCOL = (
    C.ROOT
    / "docs/evidence/dev-logical-mux-sdma-release-native-gpu1-2026-09-18/raw/native/collected/protocol.py"
)
PROTOCOL_SHA = "237ae64cecea6d64c8722813c89e6660008f49bc5d334031165ded88aae3a5a1"


def receipt(folder, command, seconds, cwd, previous, *, environment=None, stdin=None):
    need(
        {p.name for p in folder.iterdir()} == {"receipt.json", "stdout", "stderr"},
        "exact receipt files",
    )
    row = read(folder / "receipt.json")
    need(set(row) == RECEIPT_KEYS, "exact receipt keys")
    need(
        row["command"] == command
        and row["cwd"] == str(cwd)
        and row["environment"] == environment,
        "exact command, cwd and environment",
    )
    need(
        type(row["timeout_seconds"]) is int
        and row["timeout_seconds"] == seconds
        and row["stdin_sha256"]
        == (hashlib.sha256(stdin).hexdigest() if stdin is not None else None),
        "exact bound and input",
    )
    need(
        type(row["exit"]) is int
        and row["exit"] == 0
        and row["error"] is None
        and row["group_absent"] is True,
        "successful process-group closure",
    )
    need(
        type(row["pid"]) is int
        and row["pid"] > 1
        and type(row["started_ns"]) is int
        and type(row["finished_ns"]) is int
        and previous <= row["started_ns"] < row["finished_ns"],
        "strict nonoverlapping receipt chronology",
    )
    for stream in ("stdout", "stderr"):
        need(sha(folder / stream) == row[stream + "_sha256"], "stream digest")
    return row


def endpoint(folder, device):
    need((folder / "stderr").read_bytes() == b"", "empty observer stderr")
    rows = [C.parse_json(line) for line in (folder / "stdout").read_text().splitlines()]
    need(
        len(rows) == 2
        and rows[1]
        == {
            "schema": "fe2o3.copy-host-observation.v1",
            "record": "complete",
            "observations": 1,
            "refused": 0,
            "all_endpoints_admitted": True,
            "performance_accepted": False,
        },
        "one complete admitted endpoint",
    )
    value = rows[0]
    protocol = C.load_pinned(PROTOCOL, PROTOCOL_SHA, "pinned_endpoint_protocol")
    protocol.GPU, protocol.BDF, protocol.UID = device
    protocol.endpoint(value)
    observer = C.load_pinned(
        protocol.OBSERVER_SOURCE, protocol.OBSERVER_SHA, "pinned_endpoint_observer"
    )
    captures, snapshots, stamps = (
        iter([value["status"], value["pids"]]),
        iter(value["sysfs"]),
        iter([value["started"], value["finished"]]),
    )
    for snapshot in value["sysfs"]:
        need(
            set(snapshot) == {"started", "finished", "path", "values", "errors"}
            and set(snapshot["values"]) == set(observer.METRICS),
            "exact raw sysfs keys",
        )
    for captured in (value["status"], value["pids"]):
        need(
            set(captured)
            == {"command", "started", "finished", "exit", "error", "stdout", "stderr"},
            "exact raw SMI keys",
        )
    observer.capture_sysfs = lambda _root, _bdf: next(snapshots)
    observer.stamp = lambda: next(stamps)
    reconstructed = observer.observe(
        *device, Path("/opt/rocm/bin/rocm-smi"), run=lambda _command: next(captures)
    )
    reconstructed["index"] = 0
    need(
        reconstructed == value and value["endpoint_admitted"] is True,
        "raw endpoint replay",
    )


def postflight_timing(rows, name, selected):
    first, second = (str(d[0]) for d in selected)
    need(
        rows[name + "-settled-gpu" + first]["started_ns"] - rows[name]["finished_ns"]
        >= 2 * 10**9,
        "two-second settled lower bound",
    )
    need(
        rows[name + "-delayed-gpu" + first]["started_ns"]
        - rows[name + "-settled-gpu" + second]["finished_ns"]
        >= 20 * 10**9,
        "twenty-second post-settlement lower bound",
    )


def exact_tree(root, files):
    need(root.is_dir() and not root.is_symlink(), "ordinary archive root")
    directories = {
        str(parent)
        for name in files
        for parent in Path(name).parents
        if str(parent) != "."
    }
    actual_files, actual_dirs = set(), set()
    for path in root.rglob("*"):
        need(not path.is_symlink(), "no evidence symlinks")
        name = path.relative_to(root).as_posix()
        if path.is_dir():
            actual_dirs.add(name)
        else:
            sha(path)
            actual_files.add(name)
    need(
        actual_files - {"SHA256SUMS"} == set(files) and actual_dirs == directories,
        "exact file and directory membership",
    )


def verify(root=HERE):
    binding, marker = read(root / "binding.json"), read(root / "owner.json")
    need(
        set(binding)
        == {
            "schema",
            "commit",
            "cpu_seal_sha256",
            "source_files",
            "payload",
            "local_tools",
            "devices",
            "plan",
        }
        and binding["schema"] == "fe2o3.xgmi-host-attribution-experiment.v1",
        "exact binding schema",
    )
    need(
        binding["commit"] == marker["commit"]
        and marker["binding_sha256"] == sha(root / "binding.json"),
        "bound exact signed source",
    )
    owned = C.B.owned_path(marker, exists=False)
    need(
        binding["devices"] == C.devices([d[0] for d in binding["devices"]]),
        "exact known device identities",
    )
    selected = binding["devices"]
    need(
        binding["plan"]
        == {
            "order": C.PHASES,
            "bytes": 1048576,
            "depth": 1,
            "warmups": 10,
            "samples": 30,
            "settled_seconds": 2,
            "delayed_seconds": 20,
        },
        "exact workload and observation plan",
    )
    need(
        binding["local_tools"]
        == {name: sha(root / name) for name in C.STATIC_INPUTS}
        == C.committed_tools(binding["commit"]),
        "unchanged local tools",
    )
    need(
        set(binding["payload"]) == set(C.PAYLOAD)
        and binding["payload"]["campaign.py"] == sha(root / "campaign.py")
        and binding["payload"]["base.py"] == C.BASE_SHA
        and all(re.fullmatch(r"[0-9a-f]{64}", v) for v in binding["payload"].values()),
        "exact payload identities",
    )
    C.cpu_integrity()
    need(
        binding["cpu_seal_sha256"] == C.CPU_SEAL
        and binding["source_files"]
        == read(C.CPU / "raw/source-before/stdout")["files"],
        "CPU-qualified source cohort",
    )
    state = read(root / "controller-state.json")
    need(
        set(state)
        == {
            "created",
            "native_success",
            "collected",
            "cleaned",
            "absence",
            "local_payload_absent",
            "local_payload",
            "failure",
            "native_failure",
            "secondary_failures",
        }
        and all(
            state[k] is True
            for k in (
                "created",
                "native_success",
                "collected",
                "cleaned",
                "absence",
                "local_payload_absent",
            )
        )
        and state["failure"] is None
        and state["native_failure"] is None
        and state["secondary_failures"] == [],
        "complete execution, collection and cleanup",
    )
    payload = Path(state["local_payload"])
    need(
        re.fullmatch(
            r"/home/harsh/\.codex-tmp/fe2o3-xgmi-attribution-20260918\.[A-Za-z0-9_]+",
            str(payload),
        ),
        "private local payload shape",
    )
    commands = C.local_commands(payload, marker)
    need(set(commands) == set(LOCAL_ORDER), "exact local command roster")
    last, local = 0, {}
    for name in LOCAL_ORDER:
        command, seconds, stdin = commands[name]
        row = receipt(
            root / "local" / name, command, seconds, C.ROOT, last, stdin=stdin
        )
        last, local[name] = row["finished_ns"], row
    for name, count in (("calibration", 20), ("observer-tests", 19)):
        need(
            re.fullmatch(
                r"\.{"
                + str(count)
                + r"}\n-+\nRan "
                + str(count)
                + r" tests in [0-9.]+s\n\nOK\n",
                (root / "local" / name / "stderr").read_text(),
            ),
            "exact calibration closure",
        )
    need(
        (root / "local/source-clean/stdout").read_bytes() == b"",
        "clean committed source",
    )
    snapshot = read(root / "local/source/stdout")
    need(
        set(snapshot) == {"base", "files"}
        and snapshot["base"] == binding["commit"]
        and snapshot["files"] == binding["source_files"],
        "exact local source snapshot",
    )
    cpu_report = read(root / "local/cpu-verify/stdout")
    need(
        cpu_report["cpu_qualification"] is True
        and cpu_report["source_files"] == 5564
        and cpu_report["native_execution"] is False
        and cpu_report["formal_refinement"] is False
        and cpu_report["performance_acceptance"] is False,
        "CPU-only prerequisite verdict",
    )
    manifest = read(root / "remote-inventory.json")
    need(
        C.B.inventory(root / "remote") == manifest, "remote collection digest equality"
    )
    need(
        read(root / "local/create/stdout") == marker
        and read(root / "local/remote-inventory/stdout") == manifest
        and read(root / "local/cleanup/stdout") == {"removed": str(owned)}
        and read(root / "local/absence/stdout")
        == {"path_absent": True, "processes_absent": True},
        "ownership outcomes",
    )
    remote, last, receipts = root / "remote", 0, {}
    specs = C.remote_commands(owned, selected)
    for name, command, seconds in specs:
        row = receipt(
            remote / name,
            command,
            seconds,
            owned / "source",
            last,
            environment=C.environment(owned),
        )
        last, receipts[name] = row["finished_ns"], row
    for name in ("rustc", "cargo"):
        need(
            (remote / name / "stdout").read_bytes()
            == (C.CPU / "raw" / name / "stdout").read_bytes(),
            "same reported qualified Rust toolchain",
        )
    need(
        read(remote / "source-before.json")
        == binding["source_files"]
        == read(remote / "source-after.json"),
        "unchanged remote source",
    )
    binary = read(remote / "binary.json")
    need(
        set(binary) == {C.BINARY} and re.fullmatch(r"[0-9a-f]{64}", binary[C.BINARY]),
        "same-binary identity",
    )
    parsed = {}
    for name, enabled in C.PHASES:
        folder = remote / name
        need((folder / "stderr").read_bytes() == b"", "empty workload stderr")
        parsed[name] = C.parse_transcript(
            (folder / "stdout").read_bytes(), selected, enabled
        )
        for suffix in ("before", "settled", "delayed"):
            for device in selected:
                endpoint(
                    remote / (name + "-" + suffix + "-gpu" + str(device[0])), device
                )
        postflight_timing(receipts, name, selected)
    need(
        C.same_json(parsed, read(remote / "parsed.json")),
        "independent complete transcript replay",
    )
    need(
        read(remote / "finished.json")
        == {
            "commit": binding["commit"],
            "native_execution": True,
            "formal_refinement": False,
            "performance_acceptance": False,
        },
        "bounded execution verdict",
    )
    extras = {
        "source-before.json",
        "source-after.json",
        "binary.json",
        "parsed.json",
        "finished.json",
    }
    files = {
        "campaign.py",
        "test_campaign.py",
        "verify.py",
        "README.md",
        ".gitattributes",
        "binding.json",
        "owner.json",
        "remote-inventory.json",
        "controller-state.json",
    }
    files.update(
        "local/" + name + "/" + item
        for name in LOCAL_ORDER
        for item in ("receipt.json", "stdout", "stderr")
    )
    files.update(
        "remote/" + name + "/" + item
        for name, _, _ in specs
        for item in ("receipt.json", "stdout", "stderr")
    )
    files.update("remote/" + name for name in extras)
    exact_tree(root, files)
    return {
        "native_execution": True,
        "host_attribution": True,
        "formal_refinement": False,
        "performance_acceptance": False,
        "commit": binding["commit"],
        "binary_sha256": binary[C.BINARY],
        "devices": selected,
        "remote_commands": len(specs),
        "endpoint_observations": 24,
        "facade": {name: value["aggregates"] for name, value in parsed.items()},
        "stages": {name: C.summarize(parsed[name]) for name in ("on1", "on2")},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--seal", action="store_true")
    mode.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--summary", action="store_true")
    args = parser.parse_args()
    report = verify()
    manifest = "".join(
        digest + "  " + name + "\n"
        for name, digest in C.B.inventory(HERE).items()
        if name != "SHA256SUMS"
    )
    seal = HERE / "SHA256SUMS"
    if args.seal:
        with seal.open("x") as output:
            output.write(manifest)
    elif not args.allow_unsealed or seal.exists():
        need(seal.read_text() == manifest, "whole archive seal")
    if not args.summary:
        report.pop("facade")
        report.pop("stages")
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
