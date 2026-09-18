#!/usr/bin/env python3
"""Portable historical audit. Never invokes SSH, devices, Cargo or the ELF."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex

ROOT = Path(__file__).resolve().parent
COLLECTED = Path("raw/native/collected")
NEW = "/tmp/fe2o3-single-sdma-20260918.0f3uxs8l"
PAYLOAD = "c18a03390ba24e6c0e9a60f9ba44159a5e00b51a52310a3dd0d424a92e4b0954"


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def pairs(items):
    result = {}
    for key, value in items:
        need(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def parse(text):
    return json.loads(
        text,
        object_pairs_hook=pairs,
        parse_constant=lambda _: need(False, "nonfinite JSON"),
    )


def load(path):
    return parse(path.read_text())


def lines(path):
    return [parse(line) for line in path.read_text().splitlines()]


def digest_map(value):
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def module(path):
    spec = importlib.util.spec_from_file_location("retained_protocol", path)
    result = importlib.util.module_from_spec(spec)
    exec(compile(path.read_bytes(), str(path), "exec"), result.__dict__)
    return result


def record(folder, expected=None):
    row = load(folder / "record.json")
    for name in ("stdout", "stderr"):
        need(
            sha(folder / (name + ".log")) == row[name + "_sha256"],
            "command transcript hash",
        )
    if "group_absent" in row:
        need(
            row["group_absent"] is True and row["error"] is None,
            "command/group closure",
        )
    if expected is not None:
        need(
            type(row["status"]) is int and row["status"] == expected,
            "expected command status",
        )
    return row


def absence(value, path, pids):
    need(
        value["record"] == "owned-absence-observation" and value["owned"] == path,
        "absence identity",
    )
    need(
        value["recorded_processes"]
        == [
            {"pid": pid, "pid_absent": True, "process_group_absent": True}
            for pid in pids
        ],
        "exact absent PID/group roster",
    )
    need(value["accessible_references"] == [], "visible references absent")
    need(
        value["scope"]
        == "recorded-owned-PIDs/groups and accessible same-UID exe/cwd/fd/maps only; no all-user or inaccessible-reference absence claim",
        "explicit proc visibility scope",
    )
    need(
        type(value["unreadable_same_uid_entries"]) is list,
        "retained visibility limitations",
    )


def controller(value):
    need(
        value["owned"] == NEW and value["payload_sha256"] == PAYLOAD,
        "controller identity",
    )
    for name in (
        "native_attempted",
        "collected_verified",
        "cleanup_closed",
        "independent_absence_closed",
    ):
        need(value[name] is True, "controller history closure")
    need(
        value["native_outer_passed"] is True and value["failure"] is None,
        "successful campaign preserved",
    )


def verify_inventory(root, inventory, binary_sha):
    expected = inventory["files"]
    actual = {
        path.relative_to(root).as_posix(): sha(path)
        for path in root.rglob("*")
        if path.is_file()
    }
    need(
        "queue-example" not in actual and expected.get("queue-example") == binary_sha,
        "only historical ELF identity retained",
    )
    need(
        actual
        == {
            name: digest for name, digest in expected.items() if name != "queue-example"
        },
        "complete collected inventory except explicitly omitted ELF",
    )


def binary_receipt(text, binary_sha):
    need(
        text
        == binary_sha
        + "  target/x86_64-unknown-linux-musl/debug/examples/kfd-compute-aql-queue\n",
        "CPU-built non-test example executable identity",
    )


def environment(uid):
    return {
        "HOME": "/home/harsh",
        "PATH": "/usr/bin:/bin:/opt/rocm/bin",
        "PYTHONDONTWRITEBYTECODE": "1",
        "HSA_XNACK": "0",
    }


def observer_command(row, P, owned=None):
    owned = NEW if owned is None else owned
    need(
        row["command"]
        == [
            "/usr/bin/python3",
            "-B",
            owned + "/source/copy-host-observe.py",
            "--gpu-index",
            str(P.GPU),
            "--pci-bdf",
            P.BDF,
            "--unique-id",
            P.UID,
            "--samples",
            "1",
        ]
        and row["outer_bound_seconds"] == 75
        and row["environment"] == environment(P.UID),
        "exact observer argv/environment/bound",
    )


def endpoint_bracket(row, value):
    need(
        row["started"]["monotonic_ns"]
        <= value["started"]["monotonic_ns"]
        <= value["finished"]["monotonic_ns"]
        <= row["t0"]["monotonic_ns"]
        <= row["finished"]["monotonic_ns"],
        "raw observation bracketed by command clocks",
    )


def outer_command(row, owned=None):
    owned = NEW if owned is None else owned
    need(
        row["command"]
        == [
            "/usr/bin/timeout",
            "--signal=TERM",
            "--kill-after=15s",
            "1200s",
            "/usr/bin/python3",
            "-B",
            owned + "/run.py",
            "--parent-ready-approved",
        ],
        "exact native outer timeout argv",
    )


def native_transcript(P, case, stdout, stderr):
    return P.transcript(case, stdout, stderr)


def native_command(P, case, owned):
    return [
        "/usr/bin/timeout",
        "--signal=TERM",
        "--kill-after=5s",
        "180s",
        "/usr/bin/prlimit",
        "--core=0:0",
        "--fsize=16777216:16777216",
        "--",
        "/usr/bin/numactl",
        "--physcpubind=48-95",
        "--membind=1",
        owned + "/queue-example",
        "--retained-release-sdma",
        P.TESTS[case],
        P.UID,
    ]


def serial_chain(rows, names, *, native):
    previous = None
    for name in names:
        row = rows[name]
        start = row["started"]["monotonic_ns"] if native else row["started_ns"]
        end = row["finished"]["monotonic_ns"] if native else row["finished_ns"]
        need(
            type(start) is int and type(end) is int and 0 < start <= end,
            "typed ordered command interval",
        )
        need(previous is None or previous <= start, "serial command chain")
        previous = end


def cleanup_command(row, mode, pids, inventory_sha, helper_sha, owned=None):
    owned = NEW if owned is None else owned
    need(
        row["command"][:-1]
        == [
            "ssh",
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ServerAliveInterval=10",
            "-o",
            "ServerAliveCountMax=3",
            "mi300x",
        ],
        "exact cleanup SSH transport",
    )
    command = shlex.split(row["command"][-1])
    need(
        len(command) == 6
        and command[:5] == ["/usr/bin/python3", "-B", "-", mode, owned],
        "exact cleanup mode/path/argv",
    )
    expected = (
        {"pids": pids, "inventory_sha256": inventory_sha} if mode == "cleanup" else pids
    )
    need(parse(command[5]) == expected, "exact cleanup inventory/PID argument")
    need(
        row["stdin_sha256"] == helper_sha and row["bound_seconds"] == 120,
        "archived cleanup helper and bound",
    )


def manifest(root, allow_unsealed):
    path = root / "SHA256SUMS"
    if not path.exists():
        need(allow_unsealed, "unsealed packet requires --allow-unsealed")
        return False
    expected = {}
    for line in path.read_text().splitlines():
        digest, name = line.split("  ", 1)
        need(
            re.fullmatch(r"[0-9a-f]{64}", digest) is not None
            and name not in expected
            and Path(name).as_posix() == name
            and not Path(name).is_absolute()
            and ".." not in Path(name).parts,
            "manifest entry",
        )
        expected[name] = digest
    actual = {
        path.relative_to(root).as_posix(): sha(path)
        for path in root.rglob("*")
        if path.is_file() and path != root / "SHA256SUMS"
    }
    need(actual == expected, "manifest bytes and exact closure")
    return True


def rejected_attempt(root):
    rejected = root / "rejected"
    native = rejected / "raw/native"
    payload = native / "collected"
    P = module(payload / "protocol.py")
    owned = "/tmp/fe2o3-single-sdma-20260918.yxqxau8y"
    need(
        P.COMMIT == "a0ab05f38ab28b81f8a3080268d07421f118ac87",
        "rejected source identity",
    )
    need(
        sha(payload / "payload.json")
        == "a14984f90e5ddecab7de82dd8e1db1d029109c75683d7106e527b0ac0af4388f",
        "rejected frozen payload",
    )
    for name, digest in load(payload / "payload.json").items():
        if name == "queue-example":
            need(digest == P.BINARY_SHA, "rejected omitted ELF")
        else:
            need(sha(payload / name) == digest, "rejected payload bytes")
    inventory, observed = lines(native / "remote-inventory/stdout.log")
    need(inventory["owned"] == owned, "rejected inventory path")
    verify_inventory(payload, inventory, P.BINARY_SHA)
    pids = inventory["recorded_pids"]
    need(len(pids) == len(set(pids)) == 7, "rejected seven owned groups")
    absence(observed, owned, pids)
    results = payload / "results"
    names = [
        "topology",
        "placement",
        "generic-preflight",
        "generic-test",
        "generic-immediate",
        "generic-delayed",
    ]
    need(
        {p.name for p in results.iterdir() if p.is_dir()} == set(names),
        "rejected stop after first case",
    )
    rows = {
        name: record(results / name, 1 if name == "generic-test" else 0)
        for name in names
    }
    serial_chain(rows, names, native=True)
    launch = load(results / "controller-launch.json")
    outer_command(launch, owned)
    need(
        launch["status"] == 1 and launch["group_absent"] and launch["error"] is None,
        "rejected outer closed",
    )
    need(
        sorted(
            [launch["process_group"]] + [row["process_group"] for row in rows.values()]
        )
        == pids,
        "rejected original process roster",
    )
    t0 = rows["generic-test"]["t0"]["monotonic_ns"]
    test = rows["generic-test"]
    need(
        test["command"] == native_command(P, "generic", owned)
        and test["environment"] == environment(P.UID)
        and test["outer_bound_seconds"] == 200,
        "rejected exact native command/environment/bound",
    )
    for label, offset in (("preflight", None), ("immediate", 0), ("delayed", 20)):
        value, completion = lines(results / f"generic-{label}/stdout.log")
        observer_command(rows[f"generic-{label}"], P, owned)
        endpoint_bracket(rows[f"generic-{label}"], value)
        end = P.endpoint(value, t0=None if offset is None else t0, offset=offset)
        need(
            completion
            == {
                "schema": "fe2o3.copy-host-observation.v1",
                "record": "complete",
                "observations": 1,
                "refused": 0,
                "all_endpoints_admitted": True,
                "performance_accepted": False,
            },
            "rejected strict completion",
        )
        if offset is None:
            need(
                0
                <= rows["generic-test"]["spawned"]["monotonic_ns"] - end
                <= 1_000_000_000,
                "rejected fresh launch",
            )
    need(
        (results / "generic-test/stdout.log").read_text() == "",
        "no rejected success marker",
    )
    error = (results / "generic-test/stderr.log").read_text()
    need(
        re.fullmatch(
            r"\nthread 'main' \([0-9]+\) panicked at crates/fe2o3-kfd/examples/kfd-compute-aql-queue\.rs:112:5:\n"
            r"assertion `left == right` failed\n  left: 1\n right: 0\n"
            r"note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n"
            r'Error: "isolated compute-AQL queue child for unique ID 54f88318ca05093d failed with exit status: 101"\n',
            error,
        ),
        "exact rejected accounting assertion",
    )
    source = (payload / "source/kfd-compute-aql-queue.rs").read_text().splitlines()
    need(
        source[111].strip() == "assert_eq!(host_before.retained_records, 0);",
        "rejected source location before SDMA creation",
    )
    state = load(results / "campaign.json")
    need(
        state["failure"] == "ValueError: case rejected; no later case may run"
        and state["payload_after"] == "matched",
        "rejected disposition",
    )
    need(
        state["cases"]
        == [
            {
                "case": "generic",
                "test_record": "generic-test/record.json",
                "failures": ["test: ValueError: native test command passed"],
                "post_observations": {
                    "immediate": "strict_pass",
                    "delayed": "strict_pass",
                },
            }
        ],
        "rejected exact case roster",
    )
    control = load(native / "controller.json")
    need(
        control["native_outer_passed"] is False
        and control["failure"]
        == "RuntimeError: native campaign rejected; receipts and cleanup retained",
        "rejected not promoted to pass",
    )
    need(
        all(
            control[name] is True
            for name in (
                "collected_verified",
                "cleanup_closed",
                "independent_absence_closed",
            )
        ),
        "rejected cleanup closed",
    )
    for name, mode in (
        ("remote-cleanup", "cleanup"),
        ("remote-independent-absence", "absence"),
    ):
        row = record(native / name, 0)
        cleanup_command(
            row,
            mode,
            pids,
            digest_map(inventory["files"]),
            sha(rejected / "control/remote_control.py"),
            owned,
        )
    first, removed, last = lines(native / "remote-cleanup/stdout.log")
    absence(first, owned, pids)
    absence(last, owned, pids)
    need(
        removed
        == {
            "inventory_sha256": digest_map(inventory["files"]),
            "owned": owned,
            "record": "exact-owned-directory-removed",
            "removed_regular_files": 43,
        },
        "rejected exact cleanup",
    )
    observed, gone = lines(native / "remote-independent-absence/stdout.log")
    absence(observed, owned, pids)
    need(
        gone == {"absent": True, "owned": owned, "record": "independent-path-absence"},
        "rejected separate absence",
    )
    for path in (rejected / "raw").rglob("record.json"):
        record(path.parent)
    return {
        "disposition": "rejected-before-SDMA-creation",
        "strict_endpoints": 3,
        "cleanup": "closed",
        "recorded_groups_absent": 7,
    }


def audit(root, *, allow_unsealed=False, source_root=None, binary=None):
    need(not any(path.is_symlink() for path in root.rglob("*")), "no archive symlinks")
    sealed = manifest(root, allow_unsealed)
    rejected = rejected_attempt(root)
    payload = root / COLLECTED
    need(sha(payload / "payload.json") == PAYLOAD, "frozen payload identity")
    payload_map = load(payload / "payload.json")
    for name, digest in payload_map.items():
        if name != "queue-example":
            need(sha(payload / name) == digest, "frozen payload file identity")
    P = module(payload / "protocol.py")
    need(
        len(payload_map) == 20 and payload_map["queue-example"] == P.BINARY_SHA,
        "exact payload roster",
    )
    need(
        sha(payload / "cpu/SHA256SUMS") == P.CPU_SEAL
        and sha(payload / "cpu/source-before.log")
        == sha(payload / "cpu/source-after.log")
        == P.COHORT_SHA,
        "sealed final CPU source cohort identities",
    )
    cpu_manifest = {}
    for line in (payload / "cpu/SHA256SUMS").read_text().splitlines():
        digest, name = line.split("  ", 1)
        need(name not in cpu_manifest, "unique CPU manifest entries")
        cpu_manifest[name] = digest
    for name in (
        "binary.log",
        "source-before.log",
        "source-after.log",
    ):
        need(
            cpu_manifest["raw/" + name] == sha(payload / "cpu" / name),
            "retained CPU receipt bound to CPU archive seal",
        )
    binary_receipt((payload / "cpu/binary.log").read_text(), P.BINARY_SHA)
    binding = load(payload / "binding.json")
    need(
        binding["commit"] == P.COMMIT
        and binding["source_files_matched"] == 5555
        and binding["binary_sha256"] == P.BINARY_SHA
        and binding["signature_exit"] == binding["cohort_exit"] == 0,
        "historical signed source/binary binding",
    )
    cohort = load(payload / "cpu/source-before.log")["files"]
    need(len(cohort) == 5555, "source cohort cardinality")
    selected = {
        "benchmarks/runtime_gfx942/copy-host-observe.py": "copy-host-observe.py",
        "benchmarks/runtime_gfx942/r26-host-guard.py": "r26-host-guard.py",
        "crates/fe2o3-kfd/examples/kfd-compute-aql-queue.rs": "kfd-compute-aql-queue.rs",
        "crates/fe2o3-kfd/src/sdma/retained_release.rs": "retained_release.rs",
        "crates/fe2o3-kfd/src/queue_live/primary_release.rs": "primary_release.rs",
        "crates/fe2o3-kfd/src/queue_live/primary_release/driver.rs": "driver.rs",
        "docs/runtime-primary-queue-release-v1.md": "runtime-primary-queue-release-v1.md",
    }
    for source, snapshot in selected.items():
        need(
            sha(payload / "source" / snapshot) == cohort[source],
            "source snapshot/cohort identity",
        )
    signature = (payload / "commit-signature.stderr").read_text()
    need(
        "harmenon@amd.com" in signature
        and "SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg" in signature,
        "historical containing commit signature receipt",
    )
    native = root / "raw/native"
    inventory, observed = lines(native / "remote-inventory/stdout.log")
    need(
        inventory["record"] == "complete-owned-inventory" and inventory["owned"] == NEW,
        "complete native inventory identity",
    )
    verify_inventory(payload, inventory, P.BINARY_SHA)
    pids = inventory["recorded_pids"]
    need(
        len(pids) == len(set(pids)) == 15
        and all(type(pid) is int and pid > 1 for pid in pids),
        "fifteen distinct recorded owned PIDs/groups",
    )
    absence(observed, NEW, pids)
    results = payload / "results"
    rows = {}
    for folder in sorted(results.iterdir()):
        if folder.is_dir():
            rows[folder.name] = record(folder, 0)
    need(
        set(rows)
        == {"topology", "placement"}
        | {
            f"{case}-{kind}"
            for case in ("generic", "engine-0", "engine-1")
            for kind in ("preflight", "test", "immediate", "delayed")
        },
        "exact three-case command roster",
    )
    serial_chain(
        rows,
        ["topology", "placement"]
        + [
            f"{case}-{kind}"
            for case in ("generic", "engine-0", "engine-1")
            for kind in ("preflight", "test", "immediate", "delayed")
        ],
        native=True,
    )
    launch = load(results / "controller-launch.json")
    outer_command(launch)
    need(
        launch["status"] == 0
        and launch["error"] is None
        and launch["group_absent"] is True,
        "native outer success and closure",
    )
    need(
        sorted(
            [launch["process_group"]] + [row["process_group"] for row in rows.values()]
        )
        == pids,
        "PID roster independently derived from all command receipts",
    )
    need((results / "topology/stdout.log").read_text() == P.TOPOLOGY, "exact topology")
    P.placement((results / "placement/stdout.log").read_text())
    for name in rows:
        need(
            not (results / name / "stderr.log").read_text().strip(),
            "no unexpected command diagnostics",
        )
        row = rows[name]
        need(
            row["cwd"] == NEW
            and row["started"]["monotonic_ns"]
            <= row["spawned"]["monotonic_ns"]
            <= row["t0"]["monotonic_ns"]
            <= row["finished"]["monotonic_ns"],
            "complete ordered command timing",
        )
    endpoints = []
    for case in ("generic", "engine-0", "engine-1"):
        test = rows[case + "-test"]
        expected_command = native_command(P, case, NEW)
        need(
            test["command"] == expected_command and test["outer_bound_seconds"] == 200,
            "exact native test command/bounds",
        )
        need(
            test["environment"] == environment(P.UID),
            "exact parent native environment",
        )
        t0 = test["t0"]["monotonic_ns"]
        for label, offset in (("preflight", None), ("immediate", 0), ("delayed", 20)):
            value, completion = lines(results / f"{case}-{label}/stdout.log")
            observer_command(rows[f"{case}-{label}"], P)
            endpoint_bracket(rows[f"{case}-{label}"], value)
            refused = False
            need(
                completion
                == {
                    "schema": "fe2o3.copy-host-observation.v1",
                    "record": "complete",
                    "observations": 1,
                    "refused": int(refused),
                    "all_endpoints_admitted": not refused,
                    "performance_accepted": False,
                },
                "original endpoint completion disposition",
            )
            end = P.endpoint(value, t0=None if offset is None else t0, offset=offset)
            if offset is None:
                need(
                    0 <= test["spawned"]["monotonic_ns"] - end <= 1_000_000_000,
                    "fresh per-case launch",
                )
            else:
                endpoints.append(
                    {
                        "case": case,
                        "endpoint": label,
                        "start_offset_ns": value["started"]["monotonic_ns"] - t0,
                        "strict_admitted": not refused,
                    }
                )
        out = (results / f"{case}-test/stdout.log").read_text()
        err = (results / f"{case}-test/stderr.log").read_text()
        native_transcript(P, case, out, err)
    state = load(results / "campaign.json")
    need(
        [case["case"] for case in state["cases"]] == ["generic", "engine-0", "engine-1"]
        and state["commit"] == P.COMMIT
        and state["failure"] is None
        and state["payload_after"] == "matched"
        and state["native_ioctl_failure_claim"] is False
        and state["performance_claim"] is False,
        "exact successful campaign and bounded claims",
    )
    for case in state["cases"]:
        need(
            case["failures"] == []
            and case["post_observations"]
            == {"immediate": "strict_pass", "delayed": "strict_pass"}
            and case["transcript"]
            == P.transcript(
                case["case"],
                (results / (case["case"] + "-test/stdout.log")).read_text(),
                (results / (case["case"] + "-test/stderr.log")).read_text(),
            ),
            "all tests and strict observations passed",
        )
    controller(load(native / "controller.json"))
    local_names = [
        "local-protocol-tests",
        "local-payload-verify",
        "local-controller-wiring-tests",
        "remote-empty",
        "upload",
        "remote-approve",
        "native-outer",
        "remote-inventory",
        "collect",
        "remote-cleanup",
        "remote-independent-absence",
    ]
    local_summary = load(native / "controller.json")["records"]
    need(
        [row["name"] for row in local_summary] == local_names,
        "complete ordered controller receipt roster",
    )
    local_rows = {name: record(native / name, 0) for name in local_names}
    need(
        all(
            row == {"name": name, "status": 0, "error": None, "group_absent": True}
            for name, row in zip(local_names, local_summary)
        ),
        "controller summaries agree with raw receipts",
    )
    serial_chain(local_rows, local_names, native=False)
    for name, mode in (
        ("remote-cleanup", "cleanup"),
        ("remote-independent-absence", "absence"),
    ):
        cleanup_command(
            local_rows[name],
            mode,
            pids,
            digest_map(inventory["files"]),
            sha(root / "control/remote_control.py"),
        )
    cleanup = lines(native / "remote-cleanup/stdout.log")
    need(len(cleanup) == 3, "full cleanup transcript")
    absence(cleanup[0], NEW, pids)
    need(
        cleanup[1]
        == {
            "inventory_sha256": digest_map(inventory["files"]),
            "owned": NEW,
            "record": "exact-owned-directory-removed",
            "removed_regular_files": 67,
        },
        "exact collected inventory removed",
    )
    absence(cleanup[2], NEW, pids)
    final = lines(native / "remote-independent-absence/stdout.log")
    need(
        len(final) == 2
        and final[1]
        == {"absent": True, "owned": NEW, "record": "independent-path-absence"},
        "separate path absence",
    )
    absence(final[0], NEW, pids)
    for file in (root / "raw").rglob("record.json"):
        record(file.parent)
    source_count = None
    if source_root is not None:
        for name, digest in cohort.items():
            need(
                sha(source_root / name) == digest,
                "current source differs from historical cohort: " + name,
            )
        source_count = len(cohort)
    if binary is not None:
        need(sha(binary) == P.BINARY_SHA, "supplied binary differs from historical ELF")
    return {
        "archive_integrity_sealed": sealed,
        "historical_audit": "passed",
        "native_campaign": "passed",
        "profiles": ["generic", "engine-0", "engine-1"],
        "native_commands_passed": 3,
        "observations": 9,
        "strict_endpoints": 9,
        "refused_endpoints": 0,
        "post_observation_offsets": endpoints,
        "recorded_native_pids_groups_absent": len(pids),
        "cleanup": "closed",
        "current_source_files_revalidated": source_count,
        "current_binary_revalidated": binary is not None,
        "performance_claim": False,
        "real_ioctl_failure_claim": False,
        "prior_attempt": rejected,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--source-root", type=Path)
    parser.add_argument("--binary", type=Path)
    args = parser.parse_args()
    print(
        json.dumps(
            audit(
                ROOT,
                allow_unsealed=args.allow_unsealed,
                source_root=args.source_root,
                binary=args.binary,
            ),
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
