#!/usr/bin/env python3
"""Exact LogicalMux-SDMA teardown native protocol; no device access on import."""

import hashlib
import importlib.util
import json
from pathlib import Path
import re

COMMIT = "9afafa4176e8b0c6ff53f6892841924f3c7c9c27"
BINARY_SHA = "bcb2a337dd679c31726a326ddfa99e2bb72975b6bff71a7270817148cd32e109"
CPU_SEAL = "40715bbee4e76d373862261b76cbb62f94d23808cd5ecf0c457162c881d153d9"
COHORT_SHA = "b61c8b42cda67dbdeba649907cf93ca23b862b055f77616fad98fcaab19c405f"
COHORT_FILE_COUNT = 5558
SIGNERS_SHA = "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b"
OBSERVER_SHA = "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51"
TOPOLOGY_SHA = "669a25592e8ba72743bed9c0a3ced39ad392f104746c2ca04a7f2867ed8d7ca3"
GPU, UID, BDF = 2, "0xd2e26fef80cf5c33", "0000:46:00.0"
OBSERVER_SOURCE = Path(__file__).resolve().parent / "source/copy-host-observe.py"
TOPOLOGY = "topology schema=fe2o3.r26-host-topology.v1 placement=taskset-cpulist-then-numactl-physcpubind-membind-v1 gpu_index=2 pci_bdf=0000:46:00.0 unique_id=0xd2e26fef80cf5c33 numa_node=0 device_local_cpu_list=0-47 allowed_cpu_list=0-95 allowed_mem_node_list=0-1 measurement_cpu_list=0-47 observer_cpu=95 kfd_node=4 kfd_gpu_id=29122 topology_sha256=fe3f37d829f89661660b9ab1e80d453237580a84f7676bad67bf0a70158e07e6\n"
TESTS = {f"logical-mux-{lanes}": lanes for lanes in (2, 4, 8, 14, 16)}
PROFILE_SHA = "c51feb1d7e373f4f2c20c2f193b990af4892c34ab4e6ab290192a7fbb954c790"


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
    need(manifest.get("queue-example") == BINARY_SHA, "final musl binary")
    need(
        manifest.get("source/copy-host-observe.py") == OBSERVER_SHA,
        "pinned complete observer",
    )
    need(
        manifest.get("source/r26-host-guard.py") == TOPOLOGY_SHA,
        "pinned topology guard",
    )
    need(
        manifest.get("cpu/source-before.log") == COHORT_SHA,
        "final CPU source cohort",
    )
    need(
        manifest.get("cpu/source-after.log") == COHORT_SHA,
        "unchanged final CPU cohort",
    )
    need(manifest.get("cpu/SHA256SUMS") == CPU_SEAL, "sealed CPU archive")
    need(manifest.get("allowed-signers") == SIGNERS_SHA, "reviewed public signer file")
    binding = load(root / "binding.json")
    need(
        binding["commit"] == COMMIT and binding["source_files_matched"] == COHORT_FILE_COUNT,
        "signed containing source cohort",
    )
    need(
        binding["cpu_manifest_sha256"] == CPU_SEAL
        and binding["binary_sha256"] == BINARY_SHA,
        "CPU/binary binding",
    )
    return manifest


def transcript(case, stdout, stderr):
    need(case in TESTS and stdout.isascii() and stderr == "", "known clean ASCII transcript")
    lanes = TESTS[case]
    prefix = (
        "retained_logical_mux_sdma_release=complete"
        f" logical_lane_count={lanes} native_queue_count=2"
        " cursor=initial-0-no-advance-source-qualified"
        " primary_queue_id=0 sdma_queue_ids="
    )
    suffix = (
        " engine_placement=0,1 host_delta_bytes=8192 host_delta_records=2"
        " resources_returned=11 device_backing=refunded host_backing=refunded"
        " retry=rejected public_root_drop=completed packets=0 mmio_stores=0\n"
        "profile_sha256=" + PROFILE_SHA + " unique_id=" + UID[2:]
        + " queue_id=0 event_id="
    )
    tail = (
        " cwsr_shadow_pages=24 runtime=enabled-before-create-then-disabled ring=4096"
        " roles=ring,control,eop,cwsr,completion-signals gtt_policy=accepted"
        " doorbell_slice=8192 doorbell_byte_offset="
    )
    end = (
        " dontfork=confirmed mmio_stores=0 packets=0"
        " destroy=queue-then-event-then-runtime-confirmed resources_returned=11\n"
    )
    match = re.fullmatch(
        re.escape(prefix) + r"([1-9][0-9]*),([1-9][0-9]*)"
        + re.escape(suffix) + r"([1-9][0-9]*)"
        + re.escape(tail) + r"(0|[1-9][0-9]*)" + re.escape(end),
        stdout,
    )
    need(match is not None, "exact whole two-line example transcript")
    first, second, event, offset = map(int, match.groups())
    need(
        first != second
        and all(0 < value <= 0xFFFFFFFF for value in (first, second))
        and 1 <= event <= 255 and offset < 8192 and offset % 8 == 0,
        "native queue, event and doorbell observations",
    )
    return {
        "case": case,
        "logical_lane_count": lanes,
        "native_queue_count": 2,
        "engine_placement": [0, 1],
        "cursor_scope": "initial-0-no-advance-source-qualified",
        "host_delta_bytes": 8192,
        "host_delta_records": 2,
        "primary_queue_id": 0,
        "sdma_queue_ids": [first, second],
        "event_id": event,
        "doorbell_byte_offset": offset,
        "released_resources": 11,
        "public_root_drop_completed": True,
    }


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
            and parsed["mem_busy_percent"] == 0
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
        "preferred node: 0",
        "physcpubind: " + " ".join(str(cpu) for cpu in range(0, 48)),
        "cpubind: 0",
        "nodebind: 0",
        "membind: 0",
        "preferred: 0",
    ]
    need(
        [line.rstrip() for line in text.splitlines()] == expected,
        "effective NUMA/CPU placement",
    )
