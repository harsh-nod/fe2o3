#!/usr/bin/env python3
"""Portable read-only audit of the fresh, fixed-deadline HIP smoke."""

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
    1849934,
    1849935,
    1849936,
    1849989,
    1849990,
    1849992,
    1850153,
    1850972,
    1853182,
    1853416,
    1853417,
    1853418,
    1853452,
]
CSV = ",".join(map(str, PIDS))
EXPECTED = [
    ("source-export", ["python3", "-B", "freeze-source.py"]),
    (
        "create",
        [
            "bash",
            "-c",
            "ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x bash -s < create.sh",
        ],
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
    ),
    ("prepare", SSH + ["bash", f"{OWNED}/prepare.sh"]),
    ("extract-source", ["tar", "--no-same-owner", "-xf", "source.tar", "-C", "source"]),
    ("collect-prepared", ["scp", "-qr", f"mi300x:{OWNED}/results", "prepared-results"]),
    ("freeze-scripts", ["python3", "-B", "freeze-scripts.py"]),
    ("checker-tests", ["python3", "-B", "-m", "unittest", "-v", "test_check.py"]),
    ("identity-only-diff", ["python3", "-B", "review-diff.py"]),
    ("lint", ["ruff", "check", "--no-cache", *WRAPPERS]),
    ("format", ["ruff", "format", "--no-cache", "--check", *WRAPPERS]),
    (
        "shell-syntax",
        ["bash", "-n", "record.sh", "create.sh", "prepare.sh", "driver-source.sh"],
    ),
    (
        "upload-runner",
        [
            "scp",
            "-q",
            "check.py",
            "cleanup.py",
            "dependency-snapshot.py",
            "hip_copy_diagnostic.py",
            "prepare.sh",
            "protocol.py",
            "run.py",
            "topology.py",
            "scripts.sha256",
            f"mi300x:{OWNED}/",
        ],
    ),
    (
        "uploaded-identities",
        SSH
        + [
            "/usr/bin/sha256sum",
            "-c",
            f"{OWNED}/scripts.sha256",
            f"{OWNED}/results/binary.sha256",
            f"{OWNED}/results/platform.sha256",
        ],
    ),
    (
        "smoke",
        SSH + ["/usr/bin/python3", "-B", f"{OWNED}/run.py", "--parent-ready-confirmed"],
    ),
    ("collect-results", ["scp", "-qr", f"mi300x:{OWNED}/results", "returned-results"]),
    ("staging-audit", ["python3", "-B", "check.py", "returned-results"]),
    (
        "smoke-qualification",
        ["python3", "-B", "check.py", "returned-results", "--require-success"],
    ),
    (
        "cleanup",
        [
            "bash",
            "-c",
            f"ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x /usr/bin/python3 -B - --cleanup {CSV} < cleanup.py",
        ],
    ),
    (
        "independent-absence",
        [
            "bash",
            "-c",
            f"ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x /usr/bin/python3 -B - --absence {CSV} < cleanup.py",
        ],
    ),
]
PINS = {
    "run.py": "53e341556b72ae966ea83504d64c3e22985bd770c8d66dcad6e0659c2e31863e",
    "check.py": "6fb75992a6a8fa97dd5cf7a950f32bb47abb019824ff59cfd61a71152557a1b3",
    "protocol.py": "84f5949f54b65aa3ac7a764d4ae383efb9f80da49a62246fe94011eed9bdbd98",
    "cleanup.py": "ff6ff1b50b08060c6afb58c9e32db24a7567a54220c2f4a8e810a45801438b85",
    "scripts.sha256": "0490751285f493101a3caf0bc0808293fdd407263623e81036681318b2a1990a",
    "hip_copy_diagnostic.py": "d5a2ad3dc7c00e58e7666973f2fd14d1343d7dc13edb96e738c6710eb690c561",
}


def need(value, message):
    check.need(value, message)


def text(name, suffix="stdout"):
    return (HERE / "raw" / f"{name}.{suffix}").read_text()


def iso(value):
    need(
        re.fullmatch(r"2026-09-18T\d{2}:\d{2}:\d{2}\.\d{9}Z\n", value) is not None,
        "canonical UTC timestamp",
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
    for name, digest in PINS.items():
        need(
            check.digest(HERE / name) == digest,
            "pre-execution reviewed identity: " + name,
        )
    suffixes = ("command", "started", "finished", "exit", "stdout", "stderr")
    need(
        {p.name for p in (HERE / "raw").iterdir()}
        == {name + "." + suffix for name, _ in EXPECTED for suffix in suffixes},
        "exact 20 closed local receipts",
    )
    previous = ""
    for name, command in EXPECTED:
        need(shlex.split(text(name, "command")) == command, "exact command: " + name)
        need(text(name, "exit") == "0\n", "closed zero status: " + name)
        start, finish = iso(text(name, "started")), iso(text(name, "finished"))
        need(previous <= start < finish, "ordered receipt: " + name)
        previous = finish
    export = check.loads((HERE / "source-export.json").read_text())
    need(export == check.loads(text("source-export")), "source export receipt")
    need(
        export["commit"] == COMMIT
        and export["tree"] == "195abb14241e90195a6ab71d9818e8ce50d91dd9",
        "exact committed source",
    )
    need(
        export["manifest_sha256"] == check.digest(HERE / "source-files.sha256"),
        "source manifest",
    )
    need(
        export["source_tar_sha256"]
        == "09f7fa62a1a9145c3806788538c4174d7068a8d0bbcbc018b33d7f1e79d84c6a",
        "original export tar hash",
    )
    need(
        [row["path"] for row in export["files"]]
        == list(check.manifest(HERE / "source-files.sha256"))
        and len(export["files"]) == 3,
        "exact three-source roster",
    )
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
            "original Git blob",
        )
    prepared, returned = HERE / "prepared-results", HERE / "returned-results"
    build_names = {
        "binary.sha256",
        "dependencies.json",
        "hip.d",
        "hip.dynamic",
        "hip.ldd",
        "platform-built.log",
        "platform.sha256",
        "source-before.log",
        "source-built.log",
    }
    need({p.name for p in prepared.iterdir()} == build_names, "exact build roster")
    need(
        {p.name for p in returned.iterdir()} == build_names | {"smoke", "smoke.json"},
        "exact returned roster",
    )
    for path in prepared.iterdir():
        need(
            path.read_bytes() == (returned / path.name).read_bytes(),
            "unchanged prepared artifact: " + path.name,
        )
    dependencies = check.loads((prepared / "dependencies.json").read_text())
    need(
        len(dependencies) == len({row["path"] for row in dependencies}) == 350,
        "unique actual dependency roster",
    )
    need(
        {row["path"]: row["sha256"] for row in dependencies}
        == check.manifest(prepared / "platform.sha256"),
        "dependency manifest equality",
    )
    need(
        all("mock" not in row["path"].lower() for row in dependencies),
        "no mock dependency",
    )
    linkage = (prepared / "hip.ldd").read_text()
    need(
        "libamdhip64.so" in linkage
        and "libhsa-runtime64.so" in linkage
        and "not found" not in linkage,
        "real resolved HIP and ROCr linkage",
    )
    need(
        f"{BINARY_SHA}  {OWNED}/async-copy-hip\n" in text("prepare"),
        "fresh built binary identity",
    )
    need(
        "--offload-arch=gfx942" in text("prepare")
        and "cpu_affinity=48,49" in text("prepare")
        and "prepare_complete=true native_launched=false" in text("prepare"),
        "bounded CPU-only build receipt",
    )
    for name in ("source-before.log", "source-built.log"):
        need(
            (prepared / name).read_text()
            == "".join(
                path + ": OK\n" for path in check.manifest(HERE / "source-files.sha256")
            ),
            "build source identity",
        )
    need(
        (prepared / "platform-built.log").read_text()
        == "".join(
            path + ": OK\n" for path in check.manifest(prepared / "platform.sha256")
        ),
        "build platform identity",
    )
    expected_upload = "".join(
        path + ": OK\n"
        for manifest in (
            HERE / "scripts.sha256",
            prepared / "binary.sha256",
            prepared / "platform.sha256",
        )
        for path in check.manifest(manifest)
    )
    need(
        text("uploaded-identities") == expected_upload
        and text("uploaded-identities", "stderr") == "",
        "prelaunch uploaded identities",
    )
    tests = text("checker-tests", "stderr")
    need(
        re.search(r"\nRan 13 tests in [0-9.]+s\n\nOK\n\Z", tests) is not None
        and len(re.findall(r"^test_.* \.\.\. ok$", tests, re.MULTILINE)) == 13,
        "closed complete CPU tests",
    )
    need(
        check.loads(text("identity-only-diff").splitlines()[-1])
        == {
            "changed_only_path_or_build_identity": [
                "check.py",
                "protocol.py",
                "cleanup.py",
                "prepare.sh",
                "create.sh",
                "freeze-scripts.py",
            ],
            "committed_export_equal": True,
            "old_results_reused": False,
            "reviewed_scripts": 13,
        },
        "recorded identity-only comparison",
    )
    report = check.audit(returned)
    need(
        report["records"] == 13
        and report["native_launched"] is True
        and report["smoke_accepted"] is True
        and report["performance_accepted"] is False,
        "bounded native qualification only",
    )
    need(
        report
        == check.loads(text("staging-audit"))
        == check.loads(text("smoke-qualification")),
        "original audit outputs",
    )
    state = check.loads((returned / "smoke.json").read_text())
    need(state["failures"] == [], "no fixed-protocol failure")
    decoder = json.JSONDecoder(object_pairs_hook=check.unique)
    rest, objects = text("smoke"), []
    while rest.strip():
        obj, end = decoder.raw_decode(rest.lstrip())
        rest = rest.lstrip()[end:]
        objects.append(obj)
    receipts = [
        check.loads(path.read_text())
        for path in sorted((returned / "smoke").glob("*.json"))
    ]
    need(len(objects) == 14 and objects[-1] == state, "runner terminal state")
    need([row["pid"] for row in receipts] == PIDS, "actual owned PID roster")
    need(
        objects[:-1]
        == [
            {"record": row["name"], "exit": row["exit"], "error": row["error"]}
            for row in receipts
        ],
        "runner command summaries",
    )
    need(
        [row["exit"] for row in receipts] == [0] * 7 + [1] + [0] * 5,
        "original immediate refusal remains nonzero",
    )
    need(
        iso(text("smoke", "started"))
        < state["started"]["utc"]
        < state["finished"]["utc"]
        < iso(text("smoke", "finished")),
        "remote smoke inside SSH receipt",
    )
    observations = {}
    for tag in ("005-pre", "007-immediate", "008-delayed"):
        row = check.loads(
            (returned / "smoke" / (tag + ".stdout")).read_text().splitlines()[0]
        )
        observations[tag] = row
        need(row["selected_pids"] == [], "selected GPU unattached at each endpoint")
        need(
            [int(item["values"]["mem_info_vram_used"]) for item in row["sysfs"]]
            == [298647552] * 3,
            "actual direct VRAM values",
        )
        status = check.loads(row["status"]["stdout"])["card4"]
        need(
            status["VRAM Total Used Memory (B)"] == "298647552"
            and status["GPU use (%)"] == "0",
            "actual selected SMI values",
        )
    need(
        [
            int(row["values"]["gpu_busy_percent"])
            for row in observations["007-immediate"]["sysfs"]
        ]
        == [3, 0, 0],
        "actual immediate busy telemetry",
    )
    need(
        observations["007-immediate"]["reasons"] == ["sysfs-before-busy"]
        and observations["007-immediate"]["endpoint_admitted"] is False,
        "original busy-only refusal preserved",
    )
    t0 = state["native_reaped"]["monotonic_ns"]
    need(
        observations["007-immediate"]["started"]["monotonic_ns"] - t0 == 33526987,
        "actual immediate start offset",
    )
    need(
        observations["008-delayed"]["started"]["monotonic_ns"] - t0 == 20030470296,
        "actual fixed delayed start offset",
    )
    cleanup = [check.loads(line) for line in text("cleanup").splitlines()]
    absence = check.loads(text("independent-absence"))
    need(
        [row["record"] for row in cleanup]
        == ["before-cleanup-visible", "after-cleanup-visible"]
        and absence["record"] == "independent-absence",
        "cleanup and separate absence order",
    )
    for row in [*cleanup, absence]:
        need(
            row["owned"] == str(OWNED) and row["accessible_references"] == [],
            "exact owned path/reference scope",
        )
        need(
            row["scope"]
            == "recorded-owned-PIDs-and-groups-absent; accessible-reference-scan-only; not-global-reference-absence",
            "cleanup visibility limit",
        )
        need(
            row["owned_processes"]
            == [
                {"pid": pid, "pid_absent": True, "process_group_absent": True}
                for pid in PIDS
            ],
            "owned PID/group absence",
        )
        need(
            all(
                item["pid"] not in PIDS and item.get("process_group") not in PIDS
                for item in row["unreadable_unrelated_entries"]
            ),
            "unreadable processes not ours",
        )
    need(
        cleanup[1]["absent"] is True
        and cleanup[1]["removed_regular_bytes"] == 339108
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
                "no tar/cache artifacts",
            )
            with path.open("rb") as source:
                need(source.read(4) != b"\x7fELF", "no executable/library artifacts")
    print(
        json.dumps(
            {
                "portable_audit": "PASS",
                "original_files": len(origin),
                "local_receipts": len(EXPECTED),
                "remote_records": 13,
                "cpu_test_groups": 13,
                "source_files": 3,
                "dependency_files": 350,
                "native_launched": True,
                "smoke_accepted": True,
                "performance_accepted": False,
                "cleanup_absent": True,
                "scope": "one-process fixed-deadline HIP smoke, no reservation or causal attribution",
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    audit()
