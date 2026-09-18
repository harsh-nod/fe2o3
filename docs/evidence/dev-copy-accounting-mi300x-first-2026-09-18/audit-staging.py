#!/usr/bin/env python3
"""Read-only verification of this one failed attempt, without running helpers."""

import datetime
import hashlib
import json
from pathlib import Path
import re
import shlex

ROOT = Path(__file__).resolve().parent
OWNED = "/tmp/fe2o3-copy-accounting-20260918.HyOO64fz"
BINARY = "8e729393c536a7fdcb7ca42d81a667b55dc7b7163b37c5f27bdb6484698abd89"
OBSERVER = "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51"
SOURCE = "d22891250b363dda63dc154cf5a3e3a66b5bd4cd110cdc7f148c02793cd7834c"
TEST = "kfd_backend::retained_release_tests::copy_accounting::native_runtime_directional_256_mib_copy_shutdown_refunds_exact_backing"
SUFFIXES = {"command", "started", "finished", "exit", "stdout", "stderr"}


def read(path):
    return (ROOT / path).read_text()


def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def timestamp(text):
    match = re.fullmatch(r"(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d)\.(\d{9})Z\n?", text)
    assert match, text
    seconds = int(datetime.datetime.fromisoformat(match[1]).replace(tzinfo=datetime.timezone.utc).timestamp())
    return seconds * 10**9 + int(match[2])


def receipt(directory, name, status):
    prefix = f"{directory}/{name}"
    assert read(prefix + ".exit") == f"{status}\n", prefix
    started = timestamp(read(prefix + ".started"))
    finished = timestamp(read(prefix + ".finished"))
    assert started <= finished
    assert read(prefix + ".command").strip()
    return started, finished


assert digest("runtime-test") == BINARY
assert digest("copy-host-observe.py") == OBSERVER
assert digest("source-qualified-before.log") == SOURCE
source = json.loads(read("source-qualified-before.log"))
assert source["base"] == "b87f30d1b87b2dca29e9f03e8b00f99a65b04391"
assert len(source["files"]) == 5543
assert source["files"]["benchmarks/runtime_gfx942/copy-host-observe.py"] == OBSERVER
assert source["files"]["crates/fe2o3-runtime/src/kfd_backend/retained_release_tests/copy_accounting.rs"] == digest("copy_accounting.rs.txt")
for name in ("run.sh", "inspect.sh"):
    assert (ROOT / name).read_bytes() == (ROOT / "returned" / name).read_bytes()

local = {
    "source-before": 0, "create": 0, "script-syntax": 0, "local-inputs": 0,
    "upload": 0, "remote-inputs": 0, "campaign": 1, "collect": 0,
    "cleanup": 0, "absence": 0, "source-after": 0,
}
for name, status in local.items():
    receipt("raw", name, status)
    if name != "campaign":
        assert not read(f"raw/{name}.stderr")
original_raw = {f"{name}.{suffix}" for name in local for suffix in SUFFIXES}
actual_raw = {path.name for path in (ROOT / "raw").iterdir()}
assert original_raw <= actual_raw
assert actual_raw - original_raw <= {f"audit.{suffix}" for suffix in SUFFIXES}

remote = {
    "inspect-before": 0, "preflight": 0, "native": 134,
    "post-immediate": 1, "settle-delay": 0, "post-delayed": 0,
    "inspect-after": 0,
}
assert {path.name for path in (ROOT / "returned/results").iterdir()} == {
    f"{name}.{suffix}" for name in remote for suffix in SUFFIXES
} | {"outcome.txt"}
previous = 0
for name, status in remote.items():
    started, finished = receipt("returned/results", name, status)
    assert previous <= started
    previous = finished
    if name != "native":
        assert not read(f"returned/results/{name}.stderr")
assert timestamp(read("returned/results/settle-delay.finished")) - timestamp(read("returned/results/settle-delay.started")) >= 20 * 10**9
for before, after in (("campaign", "collect"), ("collect", "cleanup"), ("cleanup", "absence")):
    assert timestamp(read(f"raw/{before}.finished")) <= timestamp(read(f"raw/{after}.started"))

native = ["env", "-u", "HIP_VISIBLE_DEVICES", "-u", "ROCR_VISIBLE_DEVICES", "-u", "CUDA_VISIBLE_DEVICES", "-u", "GPU_DEVICE_ORDINAL", "FE2O3_TEST_NATIVE_ACCOUNTING=1", "FE2O3_TEST_NATIVE_UNIQUE_ID=0x54f88318ca05093d", "prlimit", "--core=0:0", "--", "timeout", "--signal=TERM", "--kill-after=5s", "180s", "numactl", "--physcpubind=48-95", "--membind=1", OWNED + "/runtime-test", TEST, "--ignored", "--exact", "--nocapture", "--test-threads=1"]
assert shlex.split(read("returned/results/native.command")) == native
assert read("returned/results/native.stdout") == f"\nrunning 1 test\ntest {TEST} ... "
stderr = read("returned/results/native.stderr")
assert "copy_accounting.rs:59:5:" in stderr
assert "  left: 4194272\n right: 4194304\n" in stderr
assert stderr.count("panicked at") == 1
assert "native_runtime_directional_copy_accounting=complete" not in stderr
assert read("returned/results/outcome.txt") == "native_launched=true native_exit=134 immediate_exit=1 delay_exit=0 delayed_exit=0 inspect_exit=0 no_retry=true\n"

observer_command = ["python3", "-B", OWNED + "/copy-host-observe.py", "--gpu-index", "4", "--pci-bdf", "0000:85:00.0", "--unique-id", "0x54f88318ca05093d", "--samples", "1"]
for name, expected, vram in (
    ("preflight", [], [298647552] * 3),
    ("post-immediate", ["sysfs-before-vram"], [567136256, 298647552, 298647552]),
    ("post-delayed", [], [298647552] * 3),
):
    assert shlex.split(read(f"returned/results/{name}.command")) == observer_command
    rows = [json.loads(line) for line in read(f"returned/results/{name}.stdout").splitlines()]
    assert len(rows) == 2
    observation, final = rows
    assert observation["record"] == "observation"
    assert observation["gpu_index"] == 4
    assert observation["pci_bdf"] == "0000:85:00.0"
    assert observation["unique_id"] == "0x54f88318ca05093d"
    assert observation["vram_limit_exclusive"] == 512 * 1024 * 1024
    assert observation["selected_pids"] == []
    assert observation["reasons"] == expected
    assert observation["endpoint_admitted"] is (not expected)
    assert [int(s["values"]["mem_info_vram_used"]) for s in observation["sysfs"]] == vram
    for snapshot in observation["sysfs"]:
        assert not snapshot["errors"]
        assert snapshot["values"]["unique_id"] == "54f88318ca05093d"
        assert snapshot["values"]["gpu_busy_percent"] == "0"
        assert snapshot["values"]["mem_busy_percent"] == "0"
    for key in ("status", "pids"):
        capture = observation[key]
        assert capture["exit"] == 0 and capture["error"] is None
        assert not capture["stderr"]
        assert timestamp(capture["started"]["utc"]) <= timestamp(capture["finished"]["utc"])
    selected = json.loads(observation["status"]["stdout"])["card4"]
    assert selected["Unique ID"] == "0x54f88318ca05093d"
    assert selected["PCI Bus"] == "0000:85:00.0"
    assert selected["GPU use (%)"] == "0"
    assert selected["VRAM Total Used Memory (B)"] == "298647552"
    pids = observation["pids"]["stdout"]
    assert "PID 3161403 is using 1 DRM device(s):\n0 \n" in pids
    assert len(re.findall(r"PID [0-9]+ is using 0 DRM device\(s\)", pids)) == 7
    assert len(re.findall(r"PID [0-9]+ is using", pids)) == 8
    assert "End of ROCm SMI Log" in pids
    assert final == {"all_endpoints_admitted": not expected, "observations": 1, "performance_accepted": False, "record": "complete", "refused": int(bool(expected)), "schema": "fe2o3.copy-host-observation.v1"}

for name in ("source-before", "source-after"):
    text = read(f"raw/{name}.stdout")
    report, _ = json.JSONDecoder().raw_decode(text)
    assert report["sourceFiles"] == 5543 and report["sourceMatchesFrozenMap"] is True
    assert report["sourceManifestSha256"] == SOURCE
    assert report["binarySha256"] == BINARY and report["observerSha256"] == OBSERVER
    assert "static-pie linked" in text and "INTERP" not in text
for name in ("inspect-before", "inspect-after"):
    text = read(f"returned/results/{name}.stdout")
    assert BINARY + "  runtime-test" in text
    assert OBSERVER + "  copy-host-observe.py" in text
    assert "numa_node=1\nlocal_cpulist=48-95\n" in text
assert "owned_exe_cwd_references=0\n" in read("raw/cleanup.stdout")
assert f"owned_directory_removed={OWNED}\n" in read("raw/cleanup.stdout")
assert f"owned_directory_absent=true owned_exe_cwd_references=0 path={OWNED}\n" in read("raw/absence.stdout")
print("PASS: one failed native attempt; 11 original local and 7 remote closed receipts; identities, retained refusal, and owned cleanup verified; no copy/refund acceptance")
