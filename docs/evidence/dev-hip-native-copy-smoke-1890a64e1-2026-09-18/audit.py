#!/usr/bin/env python3
"""Portable read-only audit of this refused, never-launched HIP smoke attempt."""

import hashlib
import json
from pathlib import Path
import re
import shlex

import check
from protocol import BINARY_SHA, COMMIT, OWNED

HERE = Path(__file__).resolve().parent
SSH = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "mi300x"]
WRAPPERS = [
    "run.py",
    "check.py",
    "protocol.py",
    "topology.py",
    "cleanup.py",
    "test_check.py",
    "freeze-scripts.py",
]
PIDS = [
    1667810,
    1667811,
    1667812,
    1667849,
    1667850,
    1667852,
    1667964,
    1667965,
    1667966,
    1667974,
]
CSV = ",".join(map(str, PIDS))
EXPECTED = [
    ("source-export", ["python3", "-B", "freeze-source.py"], 0),
    (
        "create",
        [
            "bash",
            "-c",
            "ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x bash -s < create.sh",
        ],
        0,
    ),
    (
        "upload-build",
        [
            "scp",
            "-q",
            "source.tar",
            "source-files.sha256",
            "source-export.json",
            "prepare.sh",
            "dependency-snapshot.py",
            f"mi300x:{OWNED}/",
        ],
        0,
    ),
    ("prepare", SSH + ["bash", f"{OWNED}/prepare.sh"], 0),
    (
        "collect-prepared",
        ["scp", "-qr", f"mi300x:{OWNED}/results", "prepared-results"],
        0,
    ),
    (
        "driver-source",
        [
            "bash",
            "-c",
            "ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x bash -s < driver-source.sh",
        ],
        0,
    ),
    (
        "extract-source",
        ["tar", "--no-same-owner", "-xf", "source.tar", "-C", "source"],
        0,
    ),
    ("checker-tests", ["python3", "-B", "-m", "unittest", "-v", "test_check.py"], 0),
    ("format-wrappers", ["ruff", "format", *WRAPPERS], 0),
    ("lint-wrappers", ["ruff", "check", *WRAPPERS], 1),
    ("format-tests", ["ruff", "format", "test_check.py"], 0),
    ("freeze-scripts", ["python3", "-B", "freeze-scripts.py"], 0),
    (
        "checker-full-tests",
        ["python3", "-B", "-m", "unittest", "-v", "test_check.py"],
        0,
    ),
    ("lint-final", ["ruff", "check", *WRAPPERS], 0),
    ("format-final", ["ruff", "format", "--check", *WRAPPERS], 0),
    (
        "shell-syntax",
        ["bash", "-n", "record.sh", "create.sh", "prepare.sh", "driver-source.sh"],
        0,
    ),
    (
        "upload-reviewed",
        [
            "scp",
            "-q",
            "check.py",
            "cleanup.py",
            "hip_copy_diagnostic.py",
            "protocol.py",
            "run.py",
            "topology.py",
            "scripts.sha256",
            f"mi300x:{OWNED}/",
        ],
        0,
    ),
    (
        "inspect-reviewed",
        SSH
        + [
            "/usr/bin/sha256sum",
            "-c",
            f"{OWNED}/scripts.sha256",
            f"{OWNED}/results/binary.sha256",
            f"{OWNED}/results/platform.sha256",
        ],
        0,
    ),
    (
        "smoke",
        SSH + ["/usr/bin/python3", "-B", f"{OWNED}/run.py", "--parent-ready-confirmed"],
        1,
    ),
    (
        "collect-results",
        ["scp", "-qr", f"mi300x:{OWNED}/results", "returned-results"],
        0,
    ),
    ("staging-audit", ["python3", "-B", "check.py", "returned-results"], 0),
    (
        "smoke-qualification-refusal",
        ["python3", "-B", "check.py", "returned-results", "--require-success"],
        1,
    ),
    (
        "cleanup",
        [
            "bash",
            "-c",
            f"ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x /usr/bin/python3 -B - --cleanup {CSV} < cleanup.py",
        ],
        0,
    ),
    (
        "independent-absence",
        [
            "bash",
            "-c",
            f"ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x /usr/bin/python3 -B - --absence {CSV} < cleanup.py",
        ],
        0,
    ),
]


def need(value, message):
    check.need(value, message)


def text(name, suffix="stdout"):
    return (HERE / "raw" / f"{name}.{suffix}").read_text()


def iso(value):
    need(
        re.fullmatch(r"2026-09-18T\d{2}:\d{2}:\d{2}\.\d{9}Z\n", value) is not None,
        "complete UTC timestamp",
    )
    return value.strip()


def audit():
    origin = check.manifest(HERE / "origin-files.sha256")
    for name, digest in origin.items():
        path = HERE / name
        need(
            not path.is_symlink() and check.digest(path) == digest,
            "unchanged original file: " + name,
        )
    suffixes = ["command", "started", "finished", "exit", "stdout", "stderr"]
    need(
        {path.name for path in (HERE / "raw").iterdir()}
        == {name + "." + suffix for name, _, _ in EXPECTED for suffix in suffixes},
        "closed exact 24-receipt roster",
    )
    previous = ""
    for name, command, status in EXPECTED:
        need(shlex.split(text(name, "command")) == command, "exact command: " + name)
        need(text(name, "exit") == str(status) + "\n", "exact status: " + name)
        start, finish = iso(text(name, "started")), iso(text(name, "finished"))
        need(previous <= start < finish, "ordered closed receipt: " + name)
        previous = finish
    export = check.loads((HERE / "source-export.json").read_text())
    need(export == check.loads(text("source-export")), "source export raw identity")
    need(
        export["commit"] == COMMIT
        and export["tree"] == "195abb14241e90195a6ab71d9818e8ce50d91dd9",
        "exact source commit/tree",
    )
    need(
        export["manifest_sha256"] == check.digest(HERE / "source-files.sha256"),
        "source manifest",
    )
    need(
        export["source_tar_sha256"]
        == "09f7fa62a1a9145c3806788538c4174d7068a8d0bbcbc018b33d7f1e79d84c6a",
        "original tar digest",
    )
    need(
        [row["path"] for row in export["files"]]
        == list(check.manifest(HERE / "source-files.sha256")),
        "exact three-source roster",
    )
    need(len(export["files"]) == 3, "limited source count")
    for row in export["files"]:
        content = (HERE / "source" / row["path"]).read_bytes()
        need(
            len(content) == row["bytes"]
            and hashlib.sha256(content).hexdigest() == row["sha256"],
            "original source bytes",
        )
        need(
            hashlib.sha1(
                b"blob " + str(len(content)).encode() + b"\0" + content
            ).hexdigest()
            == row["git_blob"],
            "original Git blob identity",
        )
    prepared, returned = HERE / "prepared-results", HERE / "returned-results"
    need(
        {path.name for path in prepared.iterdir()}
        == {
            "binary.sha256",
            "dependencies.json",
            "hip.d",
            "hip.dynamic",
            "hip.ldd",
            "platform-built.log",
            "platform.sha256",
            "source-before.log",
            "source-built.log",
        },
        "exact build artifact roster",
    )
    for path in prepared.iterdir():
        need(
            path.read_bytes() == (returned / path.name).read_bytes(),
            "unchanged prepared artifact: " + path.name,
        )
    dependency_rows = check.loads((prepared / "dependencies.json").read_text())
    need(
        len(dependency_rows) == 350
        and len({row["path"] for row in dependency_rows}) == 350,
        "complete actual dependency roster",
    )
    need(
        {row["path"]: row["sha256"] for row in dependency_rows}
        == check.manifest(prepared / "platform.sha256"),
        "dependency identity correspondence",
    )
    need(
        all("mock" not in row["path"].lower() for row in dependency_rows),
        "no mock dependency",
    )
    need(
        "libamdhip64.so" in (prepared / "hip.ldd").read_text()
        and "libhsa-runtime64.so" in (prepared / "hip.ldd").read_text(),
        "real HIP and ROCr linkage",
    )
    need("not found" not in (prepared / "hip.ldd").read_text(), "resolved linkage")
    need(
        f"{BINARY_SHA}  {OWNED}/async-copy-hip\n" in text("prepare"),
        "built binary identity",
    )
    need(
        "--offload-arch=gfx942" in text("prepare")
        and "cpu_affinity=48,49" in text("prepare"),
        "CPU-only explicit architecture build",
    )
    need(
        "prepare_complete=true native_launched=false" in text("prepare"),
        "build-only receipt",
    )
    need(
        "F401" in text("lint-wrappers") and "Found 1 error." in text("lint-wrappers"),
        "preserved preliminary lint failure",
    )
    for name, count in (("checker-tests", 11), ("checker-full-tests", 13)):
        output = text(name, "stderr")
        need(
            re.search(rf"\nRan {count} tests in [0-9.]+s\n\nOK\n\Z", output)
            is not None,
            "closed CPU test result",
        )
        need(
            len(re.findall(r"^test_.* \.\.\. ok$", output, re.MULTILINE)) == count,
            "complete CPU test roster",
        )
    report = check.audit(returned)
    need(
        report["records"] == 10
        and report["native_launched"] is False
        and report["smoke_accepted"] is False,
        "actual preflight refusal, no native launch",
    )
    need(
        report
        == check.loads(text("staging-audit"))
        == check.loads(text("smoke-qualification-refusal")),
        "original audit/refusal output",
    )
    state = check.loads((returned / "smoke.json").read_text())
    need(
        state["failures"] == ["pre: refused by fixed protocol"],
        "exact rejection boundary",
    )
    lines = text("smoke").splitlines()
    decoder = json.JSONDecoder(object_pairs_hook=check.unique)
    rest, objects = "\n".join(lines), []
    while rest.strip():
        obj, end = decoder.raw_decode(rest.lstrip())
        rest = rest.lstrip()[end:]
        objects.append(obj)
    need(
        len(objects) == 11 and objects[-1] == state,
        "runner output closes exact remote state",
    )
    receipts = [
        check.loads(path.read_text())
        for path in sorted((returned / "smoke").glob("*.json"))
    ]
    need([row["pid"] for row in receipts] == PIDS, "recorded owned PID roster")
    need(
        objects[:-1]
        == [
            {"record": row["name"], "exit": row["exit"], "error": row["error"]}
            for row in receipts
        ],
        "runner command summaries",
    )
    pre = check.loads((returned / "smoke/005-pre.stdout").read_text().splitlines()[0])
    need(pre["selected_pids"] == [1661511], "exact observed GPU attachment")
    need(
        [int(row["values"]["gpu_busy_percent"]) for row in pre["sysfs"]] == [1, 1, 0],
        "actual direct busy samples",
    )
    need(
        [int(row["values"]["mem_info_vram_used"]) for row in pre["sysfs"]]
        == [734560256, 633724928, 633724928],
        "actual direct VRAM samples",
    )
    cleanup = [check.loads(line) for line in text("cleanup").splitlines()]
    absence = check.loads(text("independent-absence"))
    need(
        [row["record"] for row in cleanup]
        == ["before-cleanup-visible", "after-cleanup-visible"],
        "cleanup order",
    )
    need(absence["record"] == "independent-absence", "independent absence")
    for row in [*cleanup, absence]:
        need(
            row["owned"] == str(OWNED) and row["accessible_references"] == [],
            "exact owned cleanup path",
        )
        need(
            row["scope"]
            == "recorded-owned-PIDs-and-groups-absent; accessible-reference-scan-only; not-global-reference-absence",
            "visibility limit disclosed",
        )
        need(
            row["owned_processes"]
            == [
                {"pid": pid, "pid_absent": True, "process_group_absent": True}
                for pid in PIDS
            ],
            "owned processes absent",
        )
        need(
            all(
                item["pid"] not in PIDS and item.get("process_group") not in PIDS
                for item in row["unreadable_unrelated_entries"]
            ),
            "unreadable entries not owned",
        )
    need(
        cleanup[1]["absent"] is True
        and cleanup[1]["removed_regular_bytes"] == 321997
        and absence["absent"] is True,
        "actual removal and independent absence",
    )
    need(
        cleanup[0]["utc_ns"] < cleanup[1]["utc_ns"] < absence["utc_ns"],
        "cleanup timestamps",
    )
    for path in HERE.rglob("*"):
        need(not path.is_symlink(), "no archived symlinks")
        if path.is_file():
            need(
                path.name != "source.tar"
                and not ({"__pycache__", ".ruff_cache"} & set(path.parts)),
                "no source tar or caches",
            )
            with path.open("rb") as source:
                need(source.read(4) != b"\x7fELF", "no archived executable or library")
    print(
        json.dumps(
            {
                "portable_audit": "PASS",
                "original_files": len(origin),
                "local_receipts": len(EXPECTED),
                "remote_records": 10,
                "cpu_test_groups_final": 13,
                "source_files": 3,
                "dependency_files": 350,
                "native_launched": False,
                "smoke_accepted": False,
                "performance_accepted": False,
                "cleanup_absent": True,
                "scope": "preflight refusal only; no native HIP qualification",
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    audit()
