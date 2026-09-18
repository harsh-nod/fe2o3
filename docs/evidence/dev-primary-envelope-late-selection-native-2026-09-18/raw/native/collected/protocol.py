#!/usr/bin/env python3
"""Exact R126 native case protocol; no device access on import."""

import hashlib
import importlib.util
import json
from pathlib import Path
import re

COMMIT = "a0db73625c421e26922a1bb8cab4dec49012417c"
BINARY_SHA = "001b9abd5e257098da8a27091ca81d62601805d7ca5b4ae86f81d6e69317ade2"
CPU_SEAL = "fe10e9f138029f7374a3e07d7046ca6a7bd23ca3d5c60b8876b5096a082c04d9"
COHORT_SHA = "6eff59a1d19ae0e0a761b8a7013ff8a9a192a1f44f912baf2713fd1d2dda2ca0"
OBSERVER_SHA = "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51"
TOPOLOGY_SHA = "669a25592e8ba72743bed9c0a3ced39ad392f104746c2ca04a7f2867ed8d7ca3"
GPU, UID, BDF = 4, "0x54f88318ca05093d", "0000:85:00.0"
OBSERVER_SOURCE = Path(__file__).resolve().parent / "source/copy-host-observe.py"
TOPOLOGY = "topology schema=fe2o3.r26-host-topology.v1 placement=taskset-cpulist-then-numactl-physcpubind-membind-v1 gpu_index=4 pci_bdf=0000:85:00.0 unique_id=0x54f88318ca05093d numa_node=1 device_local_cpu_list=48-95 allowed_cpu_list=0-95 allowed_mem_node_list=0-1 measurement_cpu_list=48-95 observer_cpu=47 kfd_node=6 kfd_gpu_id=53458 topology_sha256=8c7286133d538e8f9b84cdf997f6f3a343ba1b9e31ed73b0aa4ecf90561e35eb\n"
BASE = "kfd_backend::retained_release_tests::"
TESTS = {
    "positive": BASE
    + "native_runtime_typed_dispatch_shutdown_refunds_and_profiles_retained_primary",
    "error": BASE
    + "primary_envelope::native_runtime_primary_release_error_retains_installed_root",
    "panic": BASE
    + "primary_envelope::native_runtime_primary_release_panic_retains_installed_root_and_payload",
}
MARKERS = {
    "positive": "native_runtime_typed_dispatch_retained_release=complete selector=retained kernel=vecadd compute_ordinals=[0] primary_execution=confirmed pending_allocations=false packets=1 readbacks=3 output_sha256=79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3 host_account_refund=complete queue_profile=matched completed_primary_root_drop=confirmed backend_drop=completed",
    "error": "native_runtime_primary_release_envelope=error retained_original_root=true terminal=true destroyed_observation=false retry_entered=false disposition=process_exit",
    "panic": "native_runtime_primary_release_envelope=panic retained_original_root=true terminal=true destroyed_observation=false retry_entered=false original_payload=true disposition=process_exit",
}
SUMMARY = re.compile(
    r"^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 1121 filtered out; finished in [0-9]+\.[0-9]+s$",
    re.MULTILINE,
)


def need(condition, message):
    if not condition:
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
        parse_constant=lambda v: need(False, f"invalid JSON constant: {v}"),
    )


def load(path):
    return parse(path.read_text())


def payload(root):
    manifest = load(root / "payload.json")
    need(type(manifest) is dict and len(manifest) >= 12, "complete payload manifest")
    observed = set()
    for path in root.rglob("*"):
        need(not path.is_symlink(), "no payload or result symlinks")
        if path.is_file() and "results" not in path.relative_to(root).parts[:1]:
            observed.add(path.relative_to(root).as_posix())
    control = {
        name for name in ("owner.json", "approval.json") if (root / name).exists()
    }
    need(
        observed == set(manifest) | {"payload.json"} | control, "exact payload closure"
    )
    for relative, digest in manifest.items():
        path = Path(relative)
        need(
            not path.is_absolute()
            and ".." not in path.parts
            and path.as_posix() == relative,
            "normalized payload path",
        )
        full = root / path
        need(full.is_file() and not full.is_symlink(), "ordinary payload file")
        need(
            re.fullmatch(r"[0-9a-f]{64}", digest) and sha(full) == digest,
            f"payload identity: {relative}",
        )
    need(manifest.get("runtime-tests") == BINARY_SHA, "final musl binary")
    need(
        manifest.get("source/copy-host-observe.py") == OBSERVER_SHA,
        "pinned complete observer",
    )
    need(
        manifest.get("source/r26-host-guard.py") == TOPOLOGY_SHA,
        "pinned topology guard",
    )
    need(
        manifest.get("cpu/source-before-final.log") == COHORT_SHA,
        "final CPU source cohort",
    )
    need(
        manifest.get("cpu/source-after-final.log") == COHORT_SHA,
        "unchanged final CPU cohort",
    )
    need(manifest.get("cpu/SHA256SUMS") == CPU_SEAL, "sealed CPU archive")
    binding = load(root / "binding.json")
    need(
        binding["commit"] == COMMIT and binding["source_files_matched"] == 5553,
        "signed containing source cohort",
    )
    need(
        binding["cpu_manifest_sha256"] == CPU_SEAL
        and binding["binary_sha256"] == BINARY_SHA,
        "CPU/binary binding",
    )
    return manifest


def profile(text):
    lines = [
        line.split("profile_json=", 1)[1]
        for line in text.splitlines()
        if "profile_json=" in line
    ]
    need(len(lines) == 1, "one complete profiler document")
    value = parse(lines[0])
    need(
        value["schema"] == "fe2o3-kfd-runtime-profile-v1"
        and value["schema_version"] == 1,
        "profile schema",
    )
    need(
        value["device"]["target_profile"] == "gfx942:xnack-"
        and value["device"]["wave_width"] == 64,
        "profile target",
    )
    coverage, events = value["coverage"], value["events"]
    need(
        coverage["complete_runtime_operation_history"] is True
        and coverage["dropped_events"] == 0,
        "complete profiler coverage",
    )
    need(coverage["observed_events"] == len(events), "profiler event count")
    need(
        [row["sequence"] for row in events] == list(range(len(events))),
        "contiguous profiler sequence",
    )
    groups = {}
    for row in events:
        need(row["origin"] == "observed", "observed profiler origin")
        groups.setdefault(row["event"]["kind"], []).append(row)
    for name in (
        "native_queue_created",
        "dispatch_published",
        "dispatch_completed",
        "native_queue_destroyed",
    ):
        need(len(groups.get(name, [])) == 1, f"one {name}")
    created, published, completed, destroyed = [
        groups[name][0]
        for name in (
            "native_queue_created",
            "dispatch_published",
            "dispatch_completed",
            "native_queue_destroyed",
        )
    ]
    need(
        created["event"]["queue"]
        == published["event"]["queue"]
        == destroyed["event"]["queue"],
        "same primary queue identity",
    )
    need(
        published["event"]["dispatch"] == completed["event"]["dispatch"],
        "same dispatch identity",
    )
    need(
        created["sequence"]
        < published["sequence"]
        < completed["sequence"]
        < destroyed["sequence"],
        "primary event ordering",
    )
    allocations = groups.get("allocation_created", [])
    released = groups.get("allocation_released", [])
    reads = groups.get("host_read", [])
    need(
        len(allocations) == len(released) == len(reads) == 3,
        "three allocations/releases/readbacks",
    )
    identities = [row["event"]["allocation"] for row in allocations]
    need(len(set(identities)) == 3, "distinct allocation identities")
    need(
        sorted(identities)
        == sorted(row["event"]["allocation"] for row in released)
        == sorted(row["event"]["allocation"] for row in reads),
        "exact allocation identity closure",
    )
    for row in reads:
        event = row["event"]
        need(
            event["byte_offset"] == 0
            and event["content"] == {"state": "range_only", "byte_len": 4194304},
            "complete vecadd readback extent",
        )


def transcript(case, stdout, stderr):
    need(
        case in TESTS and stdout.isascii() and stderr.isascii(),
        "known ASCII transcript",
    )
    need(not stderr.strip(), "no unexpected test stderr")
    count = 1 if case == "positive" else 2
    need(
        len(re.findall(r"^running 1 test$", stdout, re.MULTILINE)) == count,
        "one-test parent/child harness count",
    )
    need(
        len(SUMMARY.findall(stdout)) == count and stdout.count("test result:") == count,
        "exact successful harness summaries",
    )
    need(
        re.findall(r"^test (\S+) \.\.\. ", stdout, re.MULTILINE)
        == [TESTS[case]] * count,
        "exact parent/child test identity",
    )
    marker_prefix = MARKERS[case].split(" ")[0].split("=")[0] + "="
    marker_lines = [
        line[line.index(marker_prefix) :]
        for line in stdout.splitlines()
        if marker_prefix in line
    ]
    need(marker_lines == [MARKERS[case]], "exact single case marker")
    need("FAILED" not in stdout and "panicked at" not in stdout, "no failed transcript")
    if case == "positive":
        need(
            "native_runtime_primary_release_envelope=" not in stdout,
            "no envelope marker in positive",
        )
        profile(stdout)
    else:
        need(
            stdout.count("native_runtime_primary_release_envelope=") == 1,
            "no extra envelope markers",
        )
        need("profile_json=" not in stdout, "no success profiler in retained case")
    return {"case": case, "harness_passes": count, "marker_count": 1}


def endpoint(value, *, t0=None, offset=None):
    need(
        value["schema"] == "fe2o3.copy-host-observation.v1"
        and value["record"] == "observation",
        "full endpoint schema",
    )
    need(
        (value["gpu_index"], value["pci_bdf"], value["unique_id"]) == (GPU, BDF, UID),
        "endpoint identity",
    )
    need(
        value["endpoint_admitted"] is True
        and value["reasons"] == []
        and value["selected_pids"] == [],
        "strict endpoint admitted",
    )
    need(
        value["vram_limit_exclusive"] == 512 * 1024 * 1024
        and value["visibility_filters"] == "removed-for-cli",
        "strict endpoint policy",
    )
    need(sha(OBSERVER_SOURCE) == OBSERVER_SHA, "pinned observer parser")
    spec = importlib.util.spec_from_file_location(
        "r126_endpoint_parser", OBSERVER_SOURCE
    )
    observer = importlib.util.module_from_spec(spec)
    exec(
        compile(OBSERVER_SOURCE.read_bytes(), str(OBSERVER_SOURCE), "exec"),
        observer.__dict__,
    )
    snapshots = value["sysfs"]
    need(len(snapshots) == 3, "three complete sysfs snapshots")
    timeline = [value["started"]]
    for item in (
        snapshots[0],
        value["status"],
        snapshots[1],
        value["pids"],
        snapshots[2],
    ):
        timeline.extend((item["started"], item["finished"]))
    timeline.append(value["finished"])
    for stamp in timeline:
        need(
            type(stamp["monotonic_ns"]) is int and stamp["monotonic_ns"] > 0,
            "typed observation timestamp",
        )
        need(
            re.fullmatch(
                r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{9}Z",
                stamp["utc"],
            )
            is not None,
            "observation UTC stamp",
        )
    need(
        all(
            left["monotonic_ns"] <= right["monotonic_ns"]
            for left, right in zip(timeline, timeline[1:])
        ),
        "complete ordered endpoint timeline",
    )
    for snapshot in snapshots:
        need(snapshot["path"] == "/sys/bus/pci/devices/" + BDF, "raw sysfs BDF path")
        need(
            not snapshot["errors"]
            and snapshot["values"]["unique_id"].lower() == UID[2:],
            "raw sysfs identity",
        )
        parsed = {
            name: observer.decimal(
                snapshot["values"][name],
                100 if name.endswith("percent") else (1 << 64) - 1,
            )
            for name in observer.METRICS
            if name != "unique_id"
        }
        need(
            parsed["gpu_busy_percent"] == 0
            and parsed["mem_info_vram_used"] < 512 * 1024 * 1024,
            "raw sysfs idle and VRAM",
        )
    for name in ("status", "pids"):
        captured = value[name]
        need(
            type(captured["exit"]) is int
            and captured["exit"] == 0
            and captured["error"] is None
            and not captured["stderr"].strip(),
            "complete raw SMI capture",
        )
        flags = (
            [
                "--showuse",
                "--showmeminfo",
                "vram",
                "--showuniqueid",
                "--showbus",
                "--json",
            ]
            if name == "status"
            else ["--showpidgpus"]
        )
        need(
            captured["command"]
            == [
                "/usr/bin/timeout",
                "--kill-after=5s",
                "20s",
                "/opt/rocm/bin/rocm-smi",
                *flags,
            ],
            "exact raw SMI command",
        )
    status = observer.parse_status(value["status"]["stdout"], GPU, BDF, UID)
    need(
        status["busy_percent"] == 0 and status["vram_bytes"] < 512 * 1024 * 1024,
        "raw SMI idle and VRAM",
    )
    need(
        not any(
            GPU in devices
            for devices in observer.parse_pids(value["pids"]["stdout"]).values()
        ),
        "raw PID attachment absence",
    )
    if t0 is not None:
        actual = value["started"]["monotonic_ns"] - t0
        need(
            offset * 1_000_000_000 <= actual <= (offset + 1) * 1_000_000_000,
            "fixed actual observer-start window",
        )
    return value["finished"]["monotonic_ns"]


def placement(text):
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
        [line.rstrip() for line in text.splitlines()] == expected,
        "effective NUMA/CPU placement",
    )
