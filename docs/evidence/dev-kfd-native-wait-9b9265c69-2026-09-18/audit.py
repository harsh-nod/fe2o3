#!/usr/bin/env python3
"""Portable audit of the actual owned campaign and cleanup, not a rerun."""

import json
from pathlib import Path
import re
import shlex

import check

HERE = Path(__file__).resolve().parent
STAGE = "/home/harsh/.codex-tmp/kfd-matched-9b9265c69-20260918.vIUYFo8l"
OWNED = check.OWNED
SSH = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "mi300x"]
SCP = ["scp", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10"]
RAW = HERE / "raw"
SUFFIXES = ("command", "started", "finished", "exit", "stdout", "stderr")
ROSTER = {
    "source-archive": [
        "git",
        "-C",
        "/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917",
        "archive",
        "--format=tar",
        "--output=" + STAGE + "/source.tar",
        check.COMMIT,
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "crates",
        "examples",
        "benchmarks/runtime_gfx942",
        "docs/evidence/dev-kfd-native-wait-mi300x-2026-09-18/summarize.py",
        "docs/evidence/dev-kfd-copy-progress-mi300x-2026-09-18/summarize.py",
    ],
    "source-freeze": ["python3", "-B", "freeze-source.py"],
    "create": [
        "bash",
        "-c",
        'ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x bash -s < "$1"',
        "--",
        STAGE + "/create.sh",
    ],
    "upload-build": SCP
    + ["source.tar", "source-files.sha256", "prepare.sh", "mi300x:" + OWNED + "/"],
    "prepare": SSH + ["bash", OWNED + "/prepare.sh"],
    "collect-prepared": [
        "scp",
        "-r",
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=10",
        "mi300x:" + OWNED + "/results",
        "prepared-results",
    ],
    "source-git-audit": ["python3", "-B", "audit-export.py"],
    "scripts-freeze": ["python3", "-B", "freeze-scripts.py"],
    "checker-tests": ["python3", "-B", "test_check.py", "-v"],
    "python-lint": [
        "ruff",
        "check",
        "--no-cache",
        "run.py",
        "check.py",
        "test_check.py",
        "cleanup.py",
        "absence.py",
        "audit-export.py",
        "freeze-scripts.py",
        "freeze-source.py",
    ],
    "python-format": [
        "ruff",
        "format",
        "--no-cache",
        "--check",
        "run.py",
        "check.py",
        "test_check.py",
        "cleanup.py",
        "absence.py",
        "audit-export.py",
        "freeze-scripts.py",
    ],
    "shell-lint": ["shellcheck", "record.sh", "create.sh", "prepare.sh"],
    "upload-runner": SCP
    + ["run.py", "check.py", "cleanup.py", "scripts.sha256", "mi300x:" + OWNED + "/"],
    "collect-binaries": SCP
    + [
        "mi300x:"
        + OWNED
        + "/target/release/examples/gfx942-runtime-directional-window-benchmark",
        "mi300x:" + OWNED + "/hsa-copy-pool-engine",
        "binaries/",
    ],
    "remote-scripts": SSH + ["sha256sum", "-c", OWNED + "/scripts.sha256"],
    "local-binaries": [
        "sha256sum",
        "binaries/gfx942-runtime-directional-window-benchmark",
        "binaries/hsa-copy-pool-engine",
    ],
    "campaign": SSH
    + ["/usr/bin/python3", "-B", OWNED + "/run.py", "--parent-ready-confirmed"],
    "collect-results": [
        "scp",
        "-r",
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=10",
        "mi300x:" + OWNED + "/results",
        "returned-results",
    ],
    "cleanup": SSH + ["/usr/bin/python3", "-B", OWNED + "/cleanup.py"],
    "cleanup-visible": ["bash", "remote-cleanup-visible.sh", "--cleanup"],
    "absence": ["bash", "remote-cleanup-visible.sh", "--absence"],
    "envelope-audit": ["python3", "-B", "check.py", "--audit", "returned-results"],
    "matched-summary": ["python3", "-B", "check.py", "--summary", "returned-results"],
    "source-git-audit-final": ["python3", "-B", "audit-export.py"],
}


def read(name, suffix):
    return (RAW / f"{name}.{suffix}").read_text()


def time_value(value):
    check.need(
        re.fullmatch(r"2026-09-18T\d{2}:\d{2}:\d{2}\.\d{9}Z\n", value) is not None,
        "closed UTC timestamp",
    )
    return value.strip()


def main():
    check.need(
        {path.name for path in RAW.iterdir()}
        == {name + "." + suffix for name in ROSTER for suffix in SUFFIXES},
        "exact closed raw receipt roster",
    )
    state = check.loads((HERE / "returned-results/campaign.json").read_text())
    for name, command in ROSTER.items():
        check.need(
            shlex.split(read(name, "command")) == command,
            "exact recorded command: " + name,
        )
        start, finish = (
            time_value(read(name, "started")),
            time_value(read(name, "finished")),
        )
        check.need(start <= finish, "closed command interval: " + name)
        expected = (
            1
            if name == "cleanup"
            else state["exit"]
            if name in ("campaign", "matched-summary")
            else 0
        )
        check.need(read(name, "exit") == str(expected) + "\n", "command exit: " + name)
    for before, after in (
        ("source-archive", "source-freeze"),
        ("source-freeze", "upload-build"),
        ("create", "upload-build"),
        ("upload-build", "prepare"),
        ("prepare", "collect-prepared"),
        ("source-git-audit", "campaign"),
        ("scripts-freeze", "checker-tests"),
        ("checker-tests", "upload-runner"),
        ("upload-runner", "remote-scripts"),
        ("remote-scripts", "campaign"),
        ("collect-binaries", "local-binaries"),
        ("local-binaries", "campaign"),
        ("campaign", "collect-results"),
        ("collect-results", "cleanup"),
        ("cleanup", "cleanup-visible"),
        ("cleanup-visible", "absence"),
        ("absence", "envelope-audit"),
        ("envelope-audit", "matched-summary"),
        ("matched-summary", "source-git-audit-final"),
    ):
        check.need(
            time_value(read(before, "finished")) <= time_value(read(after, "started")),
            "command dependency ordering",
        )
    check.need(
        time_value(read("campaign", "started"))
        <= state["started"]["utc"]
        <= state["finished"]["utc"]
        <= time_value(read("campaign", "finished")),
        "local/remote campaign envelope",
    )
    check.need(
        check.digest(HERE / "source-files.sha256")
        == "9d15e3613cc69f3a2f1437ec23c48788248b9244e81f2953da6d9db46e86c16d",
        "source manifest pin",
    )
    source = check.manifest(HERE / "source-files.sha256")
    check.need(len(source) == 5545, "complete scoped source plus validator roster")
    witness = (
        "commit="
        + check.COMMIT
        + " source_scope=5543 extra_pinned_validators=2 total_git_blobs=5545 exact_roster_and_content=true\n"
    )
    check.need(
        read("source-git-audit", "stdout").startswith(witness),
        "actual committed blob audit witness",
    )
    check.need(
        read("source-git-audit-final", "stdout") == read("source-git-audit", "stdout")
        and not read("source-git-audit-final", "stderr"),
        "final recorded committed-blob audit matches",
    )
    check.need(
        check.loads(read("source-freeze", "stdout"))
        == {
            "commit": check.COMMIT,
            "files": 5545,
            "tar_sha256": "b88884491326ade19a864f5538bf1f806f069eb2951ac134f9f6d094238ca1cd",
            "manifest_sha256": "9d15e3613cc69f3a2f1437ec23c48788248b9244e81f2953da6d9db46e86c16d",
        },
        "exact frozen export identity",
    )
    for root in (HERE / "prepared-results", HERE / "returned-results"):
        for name in ("source-before.log", "source-built.log"):
            check.need(
                (root / name).read_text()
                == "".join(path + ": OK\n" for path in source),
                "every prepared source path checked",
            )
        for name in ("binaries.sha256", "platform.sha256", "kfd.ldd", "hsa.ldd"):
            check.need(
                (root / name).read_bytes()
                == (HERE / "prepared-results" / name).read_bytes(),
                "unchanged prepared identity artifact",
            )
    check.need(
        read("remote-scripts", "stdout")
        == "".join(path + ": OK\n" for path in check.manifest(HERE / "scripts.sha256")),
        "reviewed script upload hashes",
    )
    binaries = check.manifest(HERE / "prepared-results/binaries.sha256")
    check.need(
        read("local-binaries", "stdout")
        == "".join(
            value + "  binaries/" + Path(path).name + "\n"
            for path, value in binaries.items()
        ),
        "downloaded executable identity",
    )
    check.need(
        "Ran 6 tests in " in read("checker-tests", "stderr")
        and read("checker-tests", "stderr").endswith("\nOK\n"),
        "checker CPU tests closed",
    )
    prepared = read("prepare", "stdout")
    check.need(
        "prepare_complete=true native_launched=false source_commit="
        + check.COMMIT
        + " cpu_build_jobs=2\n"
        in prepared,
        "actual build-only preparation closure",
    )
    check.need(
        "source.tar: OK\nsource-files.sha256: OK\n" in prepared,
        "transferred frozen source hashes checked",
    )
    check.need(
        "cargo_command=cargo build --frozen --release --jobs 2 -p fe2o3-runtime --features hardware-diagnostic --example gfx942-runtime-directional-window-benchmark \n"
        in prepared,
        "recorded Cargo command",
    )
    check.need(read("campaign", "stderr") == "", "runner stderr empty")
    events = [check.loads(line) for line in read("campaign", "stdout").splitlines()]
    check.need(events[-1] == state, "returned state matches direct SSH stdout")
    records = HERE / "returned-results/campaign"
    expected_events = []
    for name in state["records"]:
        row = check.loads((records / f"{name}.json").read_text())
        expected_events.append(
            {"record": name, "exit": row["exit"], "error": row["error"]}
        )
    check.need(
        events[:-1] == expected_events,
        "every direct receipt completion marker exactly once",
    )
    check.need(
        read("cleanup", "stdout") == ""
        and read("cleanup", "stderr").endswith(
            "PermissionError: [Errno 13] Permission denied: '/proc/1444849/fd'\n"
        ),
        "original cleanup refusal preserved",
    )
    cleanup = [
        check.loads(line) for line in read("cleanup-visible", "stdout").splitlines()
    ]
    check.need(
        len(cleanup) == 2
        and cleanup[0]["record"] == "before-cleanup-visible"
        and cleanup[1]["record"] == "after-cleanup-visible",
        "cleanup record roster",
    )
    check.need(
        cleanup[1]["absent"] is True and cleanup[1]["removed_regular_bytes"] > 0,
        "exact owned removal",
    )
    absent = check.loads(read("absence", "stdout"))
    check.need(
        absent["record"] == "independent-absence" and absent["absent"] is True,
        "independent owned path absence",
    )
    owned_pids = [
        check.loads((records / f"{name}.json").read_text())["pid"]
        for name in state["records"]
    ]
    for observation in [*cleanup, absent]:
        check.need(
            observation["owned"] == OWNED
            and observation["accessible_references"] == [],
            "accessible-reference scan",
        )
        check.need(
            observation["scope"]
            == "recorded-owned-PIDs-and-groups-absent; accessible-reference-scan-only; not-global-reference-absence",
            "explicit limited visibility scope",
        )
        check.need(
            observation["owned_processes"]
            == [
                {"pid": pid, "pid_absent": True, "process_group_absent": True}
                for pid in owned_pids
            ],
            "every original owned PID/group absent",
        )
        for denied in observation["unreadable_unrelated_entries"]:
            check.need(
                denied["pid"] not in owned_pids
                and denied.get("process_group") not in owned_pids,
                "unreadable entries outside recorded owned PID/groups",
            )
    check.need(
        cleanup[0]["utc_ns"] < cleanup[1]["utc_ns"] < absent["utc_ns"],
        "cleanup observation order",
    )
    report = check.audit(HERE / "returned-results", False)
    check.need(
        report["attempted_cells"] == ["1-A", "1-B", "1-D"]
        and report["native_processes"] == 3
        and report["campaign_exit"] == 1,
        "exact rejected native roster",
    )
    check.need(
        report["failure"] == "RuntimeError: sticky cell failure: 021-1-D-immediate",
        "exact retained failure",
    )
    check.need(
        len(report["endpoints"]) == 9,
        "all three complete endpoints for every launched cell",
    )
    for observation in report["endpoints"]:
        rejected = observation["key"] == "1-D" and observation["phase"] == "immediate"
        check.need(
            observation["admitted"] is (not rejected)
            and observation["reasons"] == (["sysfs-before-busy"] if rejected else [])
            and observation["selected_pids"] == [],
            "busy-only refusal and no observed selected-GPU attachment",
        )
    immediate = check.loads(
        (records / "021-1-D-immediate.stdout").read_text().splitlines()[0]
    )
    check.need(
        [row["values"]["gpu_busy_percent"] for row in immediate["sysfs"]]
        == ["46", "0", "0"],
        "exact sequential busy samples",
    )
    check.need(
        [row["values"]["mem_info_vram_used"] for row in immediate["sysfs"]]
        == ["298696704", "298647552", "298647552"],
        "exact sequential VRAM samples",
    )
    check.need(
        [row["values"]["mem_info_gtt_used"] for row in immediate["sysfs"]]
        == ["25239552", "25231360", "25231360"],
        "exact sequential GTT samples",
    )
    attachments = check.observer.parse_pids(immediate["pids"]["stdout"])
    check.need(
        {pid: devices for pid, devices in attachments.items() if devices}
        == {3161403: [0]},
        "only observed nonempty PID attachment is GPU0",
    )
    check.need(
        check.loads(read("envelope-audit", "stdout")) == report,
        "recorded envelope audit agrees",
    )
    if state["exit"] == 0:
        summary = check.audit(HERE / "returned-results", True)
        check.need(
            check.loads(read("matched-summary", "stdout")) == summary,
            "recorded matched summary agrees",
        )
    else:
        check.need(
            read("matched-summary", "stdout") == "",
            "rejected campaign emitted no metrics",
        )
    print(
        json.dumps(
            {
                "portable_audit": "PASS",
                "raw_receipts": len(ROSTER),
                "command_records": len(state["records"]),
                "source_files": len(source),
                "campaign_exit": state["exit"],
                "cleanup_absent": True,
                "scope": "guarded shared-host observations, no reservation or HIP cell",
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
