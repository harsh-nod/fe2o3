#!/usr/bin/env python3
"""Portable historical audit. Never invokes SSH, devices, Cargo or the ELF."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent
COLLECTED = Path("raw/native/collected")
NEW = "/tmp/fe2o3-r126-primary-a0db7362-20260918.mmfeMgC8"
PAYLOAD = "9f798dc90a24dc950beca265be80060f7588eff49a681dbd19e8fe8d27208201"


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
        value["native_outer_passed"] is False
        and value["failure"]
        == "RuntimeError: native campaign rejected; receipts and cleanup retained",
        "rejection preserved",
    )


def busy_refusal(P, value, *, t0):
    """Replay the pinned decision over original captures; never normalize a refusal."""
    need(sha(P.OBSERVER_SOURCE) == P.OBSERVER_SHA, "pinned refusal decision")
    observer = module(P.OBSERVER_SOURCE)
    clocks = iter((value["started"], value["finished"]))
    snapshots = iter(value["sysfs"])
    captures = iter((value["status"], value["pids"]))

    def capture_sysfs(root, bdf):
        need(
            str(root / bdf) == "/sys/bus/pci/devices/" + P.BDF, "replayed sysfs request"
        )
        snapshot = next(snapshots)
        need(snapshot["path"] == str(root / bdf), "original sysfs path")
        return snapshot

    def capture_command(command):
        captured = next(captures)
        need(captured["command"] == command, "original requested SMI command")
        return captured

    observer.stamp = lambda: next(clocks)
    observer.capture_sysfs = capture_sysfs
    reconstructed = observer.observe(
        P.GPU, P.BDF, P.UID, Path("/opt/rocm/bin/rocm-smi"), run=capture_command
    )
    need(
        next(clocks, None) is None
        and next(snapshots, None) is None
        and next(captures, None) is None,
        "complete exact capture replay",
    )
    need(
        value["index"] == 0
        and reconstructed
        == {key: item for key, item in value.items() if key != "index"},
        "original refusal independently reconstructed",
    )
    need(
        value["endpoint_admitted"] is False
        and value["reasons"] == ["sysfs-before-busy"]
        and value["selected_pids"] == []
        and [snapshot["values"]["gpu_busy_percent"] for snapshot in value["sysfs"]]
        == ["1", "0", "0"],
        "exact original one-percent busy-only refusal",
    )
    timeline = [value["started"]]
    for captured in (
        value["sysfs"][0],
        value["status"],
        value["sysfs"][1],
        value["pids"],
        value["sysfs"][2],
    ):
        timeline.extend((captured["started"], captured["finished"]))
    timeline.append(value["finished"])
    for stamp in timeline:
        need(
            type(stamp["monotonic_ns"]) is int
            and stamp["monotonic_ns"] > 0
            and re.fullmatch(
                r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{9}Z",
                stamp["utc"],
            ),
            "complete refused observation timestamp",
        )
    need(
        all(
            left["monotonic_ns"] <= right["monotonic_ns"]
            for left, right in zip(timeline, timeline[1:])
        ),
        "ordered refused observation clocks",
    )
    need(
        0 <= value["started"]["monotonic_ns"] - t0 <= 1_000_000_000,
        "refused immediate observation was in the original window",
    )
    return value["finished"]["monotonic_ns"]


def verify_inventory(root, inventory, binary_sha):
    expected = inventory["files"]
    actual = {
        path.relative_to(root).as_posix(): sha(path)
        for path in root.rglob("*")
        if path.is_file()
    }
    need(
        "runtime-tests" not in actual and expected.get("runtime-tests") == binary_sha,
        "only historical ELF identity retained",
    )
    need(
        actual
        == {
            name: digest for name, digest in expected.items() if name != "runtime-tests"
        },
        "complete collected inventory except explicitly omitted ELF",
    )


def binary_maps(before, after, binary_sha):
    need(before == after, "unchanged final CPU binary maps")
    need(set(before) == {"gnu-runtime", "musl-runtime"}, "final CPU binary roster")
    need(
        before["musl-runtime"]
        == {
            "path": "target/x86_64-unknown-linux-musl/debug/deps/fe2o3_runtime-afaad0775e07e3e7",
            "sha256": binary_sha,
        },
        "qualified musl binary identity and logical path",
    )


def environment(uid):
    return {
        "HOME": "/home/harsh",
        "PATH": "/usr/bin:/bin:/opt/rocm/bin",
        "PYTHONDONTWRITEBYTECODE": "1",
        "HSA_XNACK": "0",
        "FE2O3_TEST_NATIVE_UNIQUE_ID": uid,
    }


def observer_command(row, P):
    need(
        row["command"]
        == [
            "/usr/bin/python3",
            "-B",
            NEW + "/source/copy-host-observe.py",
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


def outer_command(row):
    need(
        row["command"]
        == [
            "/usr/bin/timeout",
            "--signal=TERM",
            "--kill-after=15s",
            "1200s",
            "/usr/bin/python3",
            "-B",
            NEW + "/run.py",
            "--parent-ready-approved",
        ],
        "exact native outer timeout argv",
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


def audit(root, *, allow_unsealed=False, source_root=None, binary=None):
    need(not any(path.is_symlink() for path in root.rglob("*")), "no archive symlinks")
    sealed = manifest(root, allow_unsealed)
    retention = load(root / "retention.json")
    for name, receipt in retention["retained"].items():
        need(sha(root / name) == receipt["sha256"], "retained original bytes")
    for receipt in retention["omitted"].values():
        if receipt["retained_elsewhere"]:
            need(
                sha(root / receipt["retained_elsewhere"]) == receipt["sha256"],
                "deduplicated original bytes",
            )
    payload = root / COLLECTED
    need(sha(payload / "payload.json") == PAYLOAD, "frozen payload identity")
    payload_map = load(payload / "payload.json")
    for name, digest in payload_map.items():
        if name != "runtime-tests":
            need(sha(payload / name) == digest, "frozen payload file identity")
    P = module(payload / "protocol.py")
    need(
        len(payload_map) == 20 and payload_map["runtime-tests"] == P.BINARY_SHA,
        "exact payload roster",
    )
    need(
        sha(payload / "cpu/SHA256SUMS") == P.CPU_SEAL
        and sha(payload / "cpu/source-before-final.log")
        == sha(payload / "cpu/source-after-final.log")
        == P.COHORT_SHA,
        "sealed final CPU source cohort identities",
    )
    cpu_manifest = {}
    for line in (payload / "cpu/SHA256SUMS").read_text().splitlines():
        digest, name = line.split("  ", 1)
        need(name not in cpu_manifest, "unique CPU manifest entries")
        cpu_manifest[name] = digest
    for name in (
        "binaries-before-final.log",
        "binaries-after-final.log",
        "source-before-final.log",
        "source-after-final.log",
    ):
        need(
            cpu_manifest["./raw/" + name] == sha(payload / "cpu" / name),
            "retained final CPU map bound to CPU archive seal",
        )
    binary_maps(
        load(payload / "cpu/binaries-before-final.log"),
        load(payload / "cpu/binaries-after-final.log"),
        P.BINARY_SHA,
    )
    binding = load(payload / "binding.json")
    need(
        binding["commit"] == P.COMMIT
        and binding["source_files_matched"] == 5553
        and binding["binary_sha256"] == P.BINARY_SHA
        and binding["signature_exit"] == binding["cohort_exit"] == 0,
        "historical signed source/binary binding",
    )
    cohort = load(payload / "cpu/source-before-final.log")["files"]
    need(len(cohort) == 5553, "source cohort cardinality")
    selected = {
        "benchmarks/runtime_gfx942/copy-host-observe.py": "copy-host-observe.py",
        "benchmarks/runtime_gfx942/r26-host-guard.py": "r26-host-guard.py",
        "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests/primary_envelope.rs": "primary_envelope.rs",
        "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests.rs": "retained_release_tests.rs",
        "crates/fe2o3-runtime/src/kfd_backend/compute_dispatch.rs": "compute_dispatch.rs",
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
            rows[folder.name] = record(
                folder, 1 if folder.name == "panic-immediate" else 0
            )
    need(
        set(rows)
        == {"topology", "placement"}
        | {
            f"{case}-{kind}"
            for case in ("positive", "error", "panic")
            for kind in ("preflight", "test", "immediate", "delayed")
        },
        "exact three-case command roster",
    )
    launch = load(results / "controller-launch.json")
    outer_command(launch)
    need(
        launch["status"] == 1
        and launch["error"] is None
        and launch["group_absent"] is True,
        "native outer rejection and closure",
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
    for case in ("positive", "error", "panic"):
        test = rows[case + "-test"]
        expected_command = [
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
            NEW + "/runtime-tests",
            "--exact",
            P.TESTS[case],
            "--ignored",
            "--nocapture",
            "--test-threads=1",
            "--color=never",
        ]
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
            refused = case == "panic" and label == "immediate"
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
            end = (
                busy_refusal(P, value, t0=t0)
                if refused
                else P.endpoint(value, t0=None if offset is None else t0, offset=offset)
            )
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
        P.transcript(case, out, err)
    state = load(results / "campaign.json")
    need(
        [case["case"] for case in state["cases"]] == ["positive", "error", "panic"]
        and state["failure"] == "ValueError: case rejected; no later case may run"
        and state["payload_after"] == "matched"
        and state["native_ioctl_failure_claim"] is False
        and state["performance_claim"] is False,
        "exact rejected campaign and claim bounds",
    )
    need(
        state["cases"][0]["failures"] == []
        and state["cases"][1]["failures"] == []
        and state["cases"][2]["failures"]
        == ["immediate: ValueError: post-observation command passed"],
        "positive/error qualification and panic observation rejection",
    )
    need(
        all(
            case["post_observations"]
            == {"immediate": "strict_pass", "delayed": "strict_pass"}
            for case in state["cases"][:2]
        )
        and state["cases"][2]["post_observations"]
        == {"immediate": "attempted", "delayed": "strict_pass"},
        "retained strict observation disposition",
    )
    controller(load(native / "controller.json"))
    for row in load(native / "controller.json")["records"]:
        record(native / row["name"], 1 if row["name"] == "native-outer" else 0)
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
    exported = load(root / "raw/prepare/export/stdout.log")
    need(
        exported["directory_mode"] == "0o700"
        and exported["payload_sha256"] == PAYLOAD
        and exported["source_files_matched"] == 5553
        and exported["binary_sha256"] == P.BINARY_SHA
        and exported["native_authorized"] is False,
        "private source-bound local export receipt",
    )
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
        "native_campaign": "rejected",
        "positive": "passed",
        "error": "passed",
        "panic": "transcript-passed-immediate-observation-refused",
        "native_commands_passed": 3,
        "harness_passes": [1, 2, 2],
        "observations": 9,
        "strict_endpoints": 8,
        "refused_endpoints": 1,
        "post_observation_offsets": endpoints,
        "recorded_native_pids_groups_absent": len(pids),
        "cleanup": "closed",
        "current_source_files_revalidated": source_count,
        "current_binary_revalidated": binary is not None,
        "performance_claim": False,
        "real_ioctl_failure_claim": False,
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
