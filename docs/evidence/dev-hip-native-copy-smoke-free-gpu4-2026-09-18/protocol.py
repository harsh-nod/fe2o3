"""Predeclared one-process HIP smoke, not a matched performance campaign."""

from pathlib import Path

OWNED = Path("/tmp/fe2o3-hip-smoke-1890a64e1-new-20260918.EvupxcTh")
COMMIT = "1890a64e1911a2a346c5a9e70a8ff5ad45b19231"
UID, BDF = "0x54f88318ca05093d", "0000:85:00.0"
SOURCE = OWNED / "source"
RESULTS = OWNED / "results"
VISIBILITY = [
    "HIP_VISIBLE_DEVICES",
    "ROCR_VISIBLE_DEVICES",
    "CUDA_VISIBLE_DEVICES",
    "GPU_DEVICE_ORDINAL",
]
OBSERVER = [
    "/usr/bin/python3",
    "-B",
    str(SOURCE / "benchmarks/runtime_gfx942/copy-host-observe.py"),
    "--gpu-index",
    "4",
    "--pci-bdf",
    BDF,
    "--unique-id",
    UID,
    "--samples",
    "1",
]
NATIVE = [
    "/usr/bin/timeout",
    "--signal=TERM",
    "--kill-after=5s",
    "180s",
    "/usr/bin/prlimit",
    "--core=0:0",
    "--",
    "/usr/bin/numactl",
    "--physcpubind=48-95",
    "--membind=1",
    str(OWNED / "async-copy-hip"),
    "0",
    "268435456",
    "1",
    "3",
    "10",
    UID,
    "diagnostic-copy-only",
]
NATIVE_ENV = {"ROCR_VISIBLE_DEVICES": "4", "HSA_XNACK": "0"}
TOPOLOGY = [
    "/usr/bin/numactl",
    "--physcpubind=48-95",
    "--membind=1",
    "/usr/bin/python3",
    "-B",
    str(OWNED / "topology.py"),
]
BUSY_ONLY = frozenset(
    ["sysfs-before-busy", "sysfs-between-busy", "sysfs-after-busy", "smi-busy"]
)
CHECKS = [
    ("source", ["/usr/bin/sha256sum", "-c", str(OWNED / "source-files.sha256")]),
    ("binary", ["/usr/bin/sha256sum", "-c", str(RESULTS / "binary.sha256")]),
    ("platform", ["/usr/bin/sha256sum", "-c", str(RESULTS / "platform.sha256")]),
    ("scripts", ["/usr/bin/sha256sum", "-c", str(OWNED / "scripts.sha256")]),
]
BINARY_SHA = "207c65b8c540e28fc0c960bd962e23e6e30802132f4a68be058d22912af076e1"
OBSERVER_SHA = "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51"
PAYLOAD_SHA = "d5a2ad3dc7c00e58e7666973f2fd14d1343d7dc13edb96e738c6710eb690c561"
IMMEDIATE_WINDOW_NS = (0, 1_000_000_000)
DELAYED_WINDOW_NS = (20_000_000_000, 21_000_000_000)
