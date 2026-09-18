#!/usr/bin/env python3
"""Exact direct-readback native protocol; no device access on import."""

import hashlib
import importlib.util
import json
from pathlib import Path
import re

COMMIT = "fd1cf3dd05e691797533b1beab1f015c4b2d8fad"
BINARY_SHA = "dbafaf826cc18b2d785015a3b4bb809c8f329f66b6f605a364a6a0e41148b145"
CPU_SEAL = "d35435e8d78df345f32697c8468a16b1aa28a7c008b845b9383a3d9c9bd2f686"
COHORT_SHA = "a4241421d6ce92b74849a0df45ea94d4fe7a2e2f13f7348b7ee7078b8c3d24f1"
OBSERVER_SHA = "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51"
TOPOLOGY_SHA = "669a25592e8ba72743bed9c0a3ced39ad392f104746c2ca04a7f2867ed8d7ca3"
GPU, UID, BDF = 4, "0x54f88318ca05093d", "0000:85:00.0"
OBSERVER_SOURCE = Path(__file__).resolve().parent / "source/copy-host-observe.py"
TOPOLOGY = "topology schema=fe2o3.r26-host-topology.v1 placement=taskset-cpulist-then-numactl-physcpubind-membind-v1 gpu_index=4 pci_bdf=0000:85:00.0 unique_id=0x54f88318ca05093d numa_node=1 device_local_cpu_list=48-95 allowed_cpu_list=0-95 allowed_mem_node_list=0-1 measurement_cpu_list=48-95 observer_cpu=47 kfd_node=6 kfd_gpu_id=53458 topology_sha256=8c7286133d538e8f9b84cdf997f6f3a343ba1b9e31ed73b0aa4ecf90561e35eb\n"
BASE = "kfd_backend::retained_release_tests::cold_allocation::"
TESTS = {
    "device": BASE + "native_runtime_cold_device_capacity_refunds_context_and_retries",
    "host": BASE + "native_runtime_cold_host_capacity_refunds_context_and_retries",
}
MARKERS = {
    "device": "native_cold_allocation_settlement=complete kind=DeviceLocal rejected_bytes=4097 retry_bytes=4096",
    "host": "native_cold_allocation_settlement=complete kind=HostVisible rejected_bytes=16781312 retry_bytes=4096",
}
READBACK_SHA = "2e7d5db52627eb273e9c3837f94bdc7bff796711c2e0960f3c0416d2cb9868d9"
SUMMARY = re.compile(
    r"^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; 1124 filtered out; finished in [0-9]+\.[0-9]+s$",
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


def transcript(case, stdout, stderr):
    need(
        case in TESTS and stdout.isascii() and stderr.isascii(),
        "known ASCII transcript",
    )
    need(not stderr.strip(), "no unexpected test stderr")
    need(
        len(re.findall(r"^running 1 test$", stdout, re.MULTILINE)) == 1,
        "one exact test harness",
    )
    need(
        len(SUMMARY.findall(stdout)) == 1 and stdout.count("test result:") == 1,
        "exact successful harness summary",
    )
    need(
        re.findall(r"^test (\S+) \.\.\. ", stdout, re.MULTILINE) == [TESTS[case]],
        "exact native test identity",
    )
    prefix = "native_cold_allocation_settlement="
    lines = [
        line[line.index(prefix) :] for line in stdout.splitlines() if prefix in line
    ]
    need(len(lines) == 1 and stdout.count(prefix) == 1, "exact single native marker")
    need(
        re.fullmatch(
            re.escape(MARKERS[case])
            + r" cold=.+ host_baseline=.+ device_baseline=.+ shutdown=.+ native_readback_sha256="
            + READBACK_SHA,
            lines[0],
        )
        is not None,
        "kind, rejection, retry and complete native readback identity",
    )
    need("FAILED" not in stdout and "panicked at" not in stdout, "no failed transcript")
    return {
        "case": case,
        "harness_passes": 1,
        "marker_count": 1,
        "readback_sha256": READBACK_SHA,
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
