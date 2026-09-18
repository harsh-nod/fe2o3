#!/usr/bin/env python3
"""Read-only verification of the rejected corrected candidate, without helpers."""

import datetime
import hashlib
import json
from pathlib import Path
import re
import shlex

ROOT = Path(__file__).resolve().parent
STAGE = "/home/harsh/.codex-tmp/kfd-accounting-corrected-20260918.FOUBOOK0"
OWNED = "/tmp/fe2o3-copy-accounting-corrected-20260918.W7d8ptkX"
BINARY = "84e91a5bd81a339d5fb74d0e4c8e5fa90e4bec0d959c0d13c42a6de0c6c2b312"
OBSERVER = "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51"
SOURCE = "fa3dfa4fa0a5faa2c92c5f84726174f308498e208af6e11b16a6a242cb8ba008"
STAGING_AUDIT = "704d4e62322d2fb768131c7c551090c88e74cd85afcc793e9ec908aab8a2aa94"
TEST = "kfd_backend::retained_release_tests::copy_accounting::native_runtime_directional_256_mib_copy_shutdown_refunds_exact_backing"
SUFFIXES = {"command", "started", "finished", "exit", "stdout", "stderr"}


def read(path):
    return (ROOT / path).read_text()


def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def timestamp(text):
    match = re.fullmatch(r"(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d)\.(\d{9})Z\n?", text)
    assert match, text
    seconds = int(
        datetime.datetime.fromisoformat(match[1])
        .replace(tzinfo=datetime.timezone.utc)
        .timestamp()
    )
    return seconds * 10**9 + int(match[2])


def receipt(directory, name, status):
    prefix = f"{directory}/{name}"
    assert read(prefix + ".exit") == f"{status}\n", prefix
    started = timestamp(read(prefix + ".started"))
    finished = timestamp(read(prefix + ".finished"))
    assert started <= finished
    assert read(prefix + ".command").strip()
    return started, finished


def command(directory, name):
    return shlex.split(read(f"{directory}/{name}.command"))


def hashes(path, expected_paths):
    lines = read(path).splitlines()
    parsed = [line.split(maxsplit=1) for line in lines]
    assert all(len(parts) == 2 for parts in parsed), path
    result = {name: value for value, name in parsed}
    assert len(lines) == len(result), f"duplicate hash path in {path}"
    assert set(result) == set(expected_paths), path
    return result


assert not (ROOT / "runtime-test").exists(), "large ELF is intentionally omitted"
local_paths = [
    "runtime-test",
    "copy-host-observe.py",
    "source-before.log",
    "copy_accounting.rs.txt",
    "record.sh",
    "source-check.sh",
    "create.sh",
    "run.sh",
    "inspect.sh",
    "cleanup.sh",
    "absence.sh",
]
local_hashes = hashes("raw/local-inputs.stdout", local_paths)
assert local_hashes.pop("runtime-test") == BINARY
for path, expected in local_hashes.items():
    assert digest(path) == expected, path
remote_paths = [
    OWNED + "/runtime-test",
    OWNED + "/copy-host-observe.py",
    OWNED + "/run.sh",
    OWNED + "/inspect.sh",
]
remote_hashes = hashes("raw/remote-inputs.stdout", remote_paths)
assert remote_hashes == {
    OWNED + "/runtime-test": BINARY,
    OWNED + "/copy-host-observe.py": digest("copy-host-observe.py"),
    OWNED + "/run.sh": digest("run.sh"),
    OWNED + "/inspect.sh": digest("inspect.sh"),
}
assert digest("copy-host-observe.py") == OBSERVER
assert digest("source-before.log") == SOURCE
source = json.loads(read("source-before.log"))
assert source["base"] == "b87f30d1b87b2dca29e9f03e8b00f99a65b04391"
assert len(source["files"]) == 5543
assert source["files"]["benchmarks/runtime_gfx942/copy-host-observe.py"] == OBSERVER
assert source["files"][
    "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests/copy_accounting.rs"
] == digest("copy_accounting.rs.txt")
assert (
    digest("copy_accounting.rs.txt")
    == "bded7480cdf174d91756b5cbf1566aee8acd7da9af122cbc9bbea0ba5925cdf8"
)
for name in ("run.sh", "inspect.sh"):
    assert (ROOT / name).read_bytes() == (ROOT / "returned" / name).read_bytes()
assert read("returned/owner") == (
    "fe2o3-copy-accounting-"
    "84e91a5bd81a339d5fb74d0e4c8e5fa90e4bec0d959c0d13c42a6de0c6c2b312\n"
)

local = {
    "source-before": 0,
    "create": 0,
    "script-syntax": 0,
    "local-inputs": 0,
    "upload": 0,
    "remote-inputs": 0,
    "campaign": 1,
    "early-native": 0,
    "collect": 0,
    "cleanup": 0,
    "absence": 0,
    "source-after": 0,
}
for name, status in local.items():
    receipt("raw", name, status)
    if name != "campaign":
        assert not read(f"raw/{name}.stderr")
original_raw = {f"{name}.{suffix}" for name in local for suffix in SUFFIXES}
actual_raw = {path.name for path in (ROOT / "raw").iterdir()}
assert actual_raw == original_raw | {f"audit.{suffix}" for suffix in SUFFIXES}

ssh_prefix = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "mi300x"]
ssh_stdin = 'ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x bash -s < "$1"'
assert command("raw", "source-before") == ["bash", "source-check.sh"]
assert command("raw", "source-after") == ["bash", "source-check.sh"]
assert command("raw", "create") == [
    "bash",
    "-c",
    ssh_stdin,
    "--",
    STAGE + "/create.sh",
]
assert command("raw", "script-syntax") == [
    "bash",
    "-n",
    "run.sh",
    "inspect.sh",
    "cleanup.sh",
    "absence.sh",
]
assert command("raw", "local-inputs") == ["sha256sum", *local_paths]
assert command("raw", "upload") == [
    "scp",
    "-o",
    "BatchMode=yes",
    "-o",
    "ConnectTimeout=10",
    "runtime-test",
    "copy-host-observe.py",
    "run.sh",
    "inspect.sh",
    "mi300x:" + OWNED + "/",
]
assert command("raw", "remote-inputs") == [*ssh_prefix, "sha256sum", *remote_paths]
assert command("raw", "campaign") == [*ssh_prefix, "bash", OWNED + "/run.sh"]
assert command("raw", "collect") == [
    "scp",
    "-r",
    "-o",
    "BatchMode=yes",
    "-o",
    "ConnectTimeout=10",
    "mi300x:" + OWNED + "/results",
    "mi300x:" + OWNED + "/owner",
    "mi300x:" + OWNED + "/run.sh",
    "mi300x:" + OWNED + "/inspect.sh",
    "returned/",
]
for name in ("cleanup", "absence"):
    assert command("raw", name) == [
        "bash",
        "-c",
        ssh_stdin,
        "--",
        STAGE + f"/{name}.sh",
    ]
assert command("raw", "audit") == ["python3", "-B", "audit.py"]
assert receipt("raw", "audit", 0)
assert digest("audit-staging.py") == STAGING_AUDIT
assert not read("raw/audit.stderr")
assert read("raw/audit.stdout") == (
    "PASS: rejected corrected candidate; 12 original local and 7 remote closed "
    "receipts; frozen identities, early native output, sticky refusal and owned "
    "cleanup verified; explicit copy/readback/refund not reached\n"
)

remote = {
    "inspect-before": 0,
    "preflight": 0,
    "native": 134,
    "post-immediate": 1,
    "settle-delay": 0,
    "post-delayed": 0,
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
assert (
    timestamp(read("returned/results/settle-delay.finished"))
    - timestamp(read("returned/results/settle-delay.started"))
    >= 20 * 10**9
)
for before, after in (("campaign", "collect"), ("collect", "cleanup")):
    assert timestamp(read(f"raw/{before}.finished")) <= timestamp(
        read(f"raw/{after}.started")
    )
assert timestamp(read("raw/cleanup.started")) <= timestamp(
    read("raw/source-after.started")
)
assert timestamp(read("raw/source-after.finished")) <= timestamp(
    read("raw/absence.started")
)
assert timestamp(read("raw/cleanup.finished")) <= timestamp(read("raw/absence.started"))

native = [
    "env",
    "-u",
    "HIP_VISIBLE_DEVICES",
    "-u",
    "ROCR_VISIBLE_DEVICES",
    "-u",
    "CUDA_VISIBLE_DEVICES",
    "-u",
    "GPU_DEVICE_ORDINAL",
    "FE2O3_TEST_NATIVE_ACCOUNTING=1",
    "FE2O3_TEST_NATIVE_UNIQUE_ID=0x54f88318ca05093d",
    "prlimit",
    "--core=0:0",
    "--",
    "timeout",
    "--signal=TERM",
    "--kill-after=5s",
    "180s",
    "numactl",
    "--physcpubind=48-95",
    "--membind=1",
    OWNED + "/runtime-test",
    TEST,
    "--ignored",
    "--exact",
    "--nocapture",
    "--test-threads=1",
]
assert shlex.split(read("returned/results/native.command")) == native
assert read("returned/results/native.stdout") == f"\nrunning 1 test\ntest {TEST} ... "
stderr = read("returned/results/native.stderr")
assert "copy_accounting.rs:96:9:" in stderr
assert "  left: (0, 3, 0)\n right: (0, 0, 0)\n" in stderr
assert stderr.count("panicked at") == 1
assert "native_runtime_directional_copy_accounting=complete" not in stderr
assert (
    read("raw/early-native.stdout") == read("returned/results/native.stdout") + stderr
)
assert command("raw", "early-native") == [
    *ssh_prefix,
    "cat",
    OWNED + "/results/native.stdout",
    OWNED + "/results/native.stderr",
]
assert timestamp(read("returned/results/native.finished")) <= timestamp(
    read("raw/early-native.started")
)
assert timestamp(read("raw/early-native.finished")) <= timestamp(
    read("raw/campaign.finished")
)
assert (
    read("returned/results/outcome.txt")
    == "native_launched=true native_exit=134 immediate_exit=1 delay_exit=0 delayed_exit=0 inspect_exit=0 no_retry=true\n"
)

observer_command = [
    "python3",
    "-B",
    OWNED + "/copy-host-observe.py",
    "--gpu-index",
    "4",
    "--pci-bdf",
    "0000:85:00.0",
    "--unique-id",
    "0x54f88318ca05093d",
    "--samples",
    "1",
]
for name, expected, vram in (
    ("preflight", [], [298647552] * 3),
    ("post-immediate", ["sysfs-before-vram"], [567128064, 298647552, 298647552]),
    ("post-delayed", [], [298647552] * 3),
):
    assert command("returned/results", name) == observer_command
    rows = [
        json.loads(line)
        for line in read(f"returned/results/{name}.stdout").splitlines()
    ]
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
    assert [
        int(s["values"]["mem_info_vram_used"]) for s in observation["sysfs"]
    ] == vram
    points = [observation["started"]]
    for snapshot in observation["sysfs"]:
        assert not snapshot["errors"]
        assert snapshot["path"] == "/sys/bus/pci/devices/0000:85:00.0"
        assert snapshot["values"]["unique_id"] == "54f88318ca05093d"
        assert snapshot["values"]["gpu_busy_percent"] == "0"
        assert snapshot["values"]["mem_busy_percent"] == "0"
        points.extend([snapshot["started"], snapshot["finished"]])
    for key in ("status", "pids"):
        capture = observation[key]
        assert capture["exit"] == 0 and capture["error"] is None
        assert not capture["stderr"]
        assert timestamp(capture["started"]["utc"]) <= timestamp(
            capture["finished"]["utc"]
        )
    points.append(observation["finished"])
    assert all(
        earlier["monotonic_ns"] <= later["monotonic_ns"]
        for earlier, later in zip(points, points[1:])
    )
    smi = [
        "/usr/bin/timeout",
        "--kill-after=5s",
        "20s",
        "/opt/rocm/bin/rocm-smi",
    ]
    assert observation["status"]["command"] == smi + [
        "--showuse",
        "--showmeminfo",
        "vram",
        "--showuniqueid",
        "--showbus",
        "--json",
    ]
    assert observation["pids"]["command"] == smi + ["--showpidgpus"]
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
    assert final == {
        "all_endpoints_admitted": not expected,
        "observations": 1,
        "performance_accepted": False,
        "record": "complete",
        "refused": int(bool(expected)),
        "schema": "fe2o3.copy-host-observation.v1",
    }

assert command("returned/results", "settle-delay") == ["sleep", "20"]
for name in ("inspect-before", "inspect-after"):
    assert command("returned/results", name) == ["bash", OWNED + "/inspect.sh"]
assert read("raw/source-before.stdout") == read("raw/source-after.stdout")
for name in ("source-before", "source-after"):
    text = read(f"raw/{name}.stdout")
    report, _ = json.JSONDecoder().raw_decode(text)
    assert report["sourceFiles"] == 5543 and report["sourceMatchesFrozenMap"] is True
    assert report["sourceManifestSha256"] == SOURCE
    assert report["binarySha256"] == BINARY and report["observerSha256"] == OBSERVER
    assert report["testSourceSha256"] == digest("copy_accounting.rs.txt")
    assert "static-pie linked" in text and "INTERP" not in text
for name in ("inspect-before", "inspect-after"):
    text = read(f"returned/results/{name}.stdout")
    assert BINARY + "  runtime-test" in text
    assert OBSERVER + "  copy-host-observe.py" in text
    assert "numa_node=1\nlocal_cpulist=48-95\n" in text
assert "owned_exe_cwd_references=0\n" in read("raw/cleanup.stdout")
assert f"owned_directory_removed={OWNED}\n" in read("raw/cleanup.stdout")
assert f"owned_directory_absent=true owned_exe_cwd_references=0 path={OWNED}\n" in read(
    "raw/absence.stdout"
)

shellcheck_paths = [
    "absence.sh",
    "cleanup.sh",
    "create.sh",
    "inspect.sh",
    "record.sh",
    "returned/inspect.sh",
    "returned/run.sh",
    "run.sh",
    "source-check.sh",
    "portable-record.sh",
    "seal.sh",
]
portable_commands = {
    "lint": ["ruff", "check", "audit.py"],
    "format": ["ruff", "format", "--check", "audit.py"],
    "shellcheck": ["shellcheck", *shellcheck_paths],
    "audit": ["python3", "-B", "audit.py"],
}
portable_actual = {path.name for path in (ROOT / "portable").iterdir()}
portable_expected = {
    f"{name}.{suffix}" for name in portable_commands for suffix in SUFFIXES
}
audit_pending = not (ROOT / "portable/audit.exit").exists()
if audit_pending:
    portable_expected -= {"audit.exit", "audit.finished"}
assert portable_actual == portable_expected
for name, expected in portable_commands.items():
    assert command("portable", name) == expected
    if name == "audit" and audit_pending:
        timestamp(read("portable/audit.started"))
        continue
    receipt("portable", name, 0)
    assert not read(f"portable/{name}.stderr")

print(
    "PASS: rejected corrected candidate; 12 original local and 7 remote closed "
    "receipts; frozen identities, early native output, sticky refusal and owned "
    "cleanup verified; explicit copy/readback/refund not reached"
)
