#!/usr/bin/env python3
"""Read-only portable audit of one native copy and accounting success."""

import datetime
import hashlib
import json
from pathlib import Path
import re
import shlex

ROOT = Path(__file__).resolve().parent
STAGE = "/home/harsh/.codex-tmp/kfd-accounting-live-credits-20260918.oZTE2ouG"
OWNED = "/tmp/fe2o3-copy-accounting-live-credits-20260918.UYReiiVM"
BINARY = "f92bbb2ec17040fff2745f2af7e897fb9488b6e6ef1fb240f98fedacf39c86fe"
OBSERVER = "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51"
SOURCE = "1a704996dc08cca9900e3034e3bed15b11ee9f5a7a6e0b049704ca8f4e0a79e9"
TEST = "kfd_backend::retained_release_tests::copy_accounting::native_runtime_directional_256_mib_copy_shutdown_refunds_exact_backing"
SUFFIXES = {"command", "started", "finished", "exit", "stdout", "stderr"}


def read(path):
    return (ROOT / path).read_text()


def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def unique_object(pairs):
    result = {}
    for name, value in pairs:
        assert name not in result, f"duplicate JSON key: {name}"
        result[name] = value
    return result


def load_json(text):
    return json.loads(text, object_pairs_hook=unique_object)


def pid_records(text):
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    assert len(lines) >= 4
    assert re.fullmatch(r"=+ ROCm System Management Interface =+", lines[0])
    assert re.fullmatch(r"=+ GPUs Indexed by PID =+", lines[1])
    assert re.fullmatch(r"=+", lines[-2])
    assert re.fullmatch(r"=+ End of ROCm SMI Log =+", lines[-1])
    rows, index, result = lines[2:-2], 0, {}
    while index < len(rows):
        match = re.fullmatch(
            r"PID ([1-9][0-9]*) is using ([0-9]+) DRM device\(s\)(:?)", rows[index]
        )
        assert match
        pid, count = int(match[1]), int(match[2])
        assert pid not in result and bool(count) == bool(match[3])
        index += 1
        devices = []
        if count:
            assert index < len(rows) and re.fullmatch(
                r"[0-9]+(?:\s+[0-9]+)*", rows[index]
            )
            devices = [int(value) for value in rows[index].split()]
            index += 1
            assert len(devices) == count and len(set(devices)) == count
        result[pid] = devices
    return result


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
    assert all(re.fullmatch(r"[0-9a-f]{64}", parts[0]) for parts in parsed), path
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
source = load_json(read("source-before.log"))
assert source["base"] == "b87f30d1b87b2dca29e9f03e8b00f99a65b04391"
assert len(source["files"]) == 5543
assert source["files"]["benchmarks/runtime_gfx942/copy-host-observe.py"] == OBSERVER
assert source["files"][
    "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests/copy_accounting.rs"
] == digest("copy_accounting.rs.txt")
assert (
    digest("copy_accounting.rs.txt")
    == "b42daaf3293afc6da164d3c0c6c5a354b12d33fb795e4317789a7938ac79a4b9"
)
for name in ("run.sh", "inspect.sh"):
    assert (ROOT / name).read_bytes() == (ROOT / "returned" / name).read_bytes()
assert read("returned/owner") == (
    "fe2o3-copy-accounting-"
    "f92bbb2ec17040fff2745f2af7e897fb9488b6e6ef1fb240f98fedacf39c86fe\n"
)

local = {
    "source-before": 0,
    "create": 0,
    "script-syntax": 0,
    "local-inputs": 0,
    "upload": 0,
    "remote-inputs": 0,
    "campaign": 0,
    "collect": 0,
    "cleanup": 0,
    "absence": 0,
    "source-after": 0,
}
for name, status in local.items():
    receipt("raw", name, status)
    assert not read(f"raw/{name}.stderr")
original_raw = {f"{name}.{suffix}" for name in local for suffix in SUFFIXES}
actual_raw = {path.name for path in (ROOT / "raw").iterdir()}
assert actual_raw == original_raw

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
remote = {
    "inspect-before": 0,
    "preflight": 0,
    "native": 0,
    "post-immediate": 0,
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
assert timestamp(read("raw/campaign.finished")) <= timestamp(
    read("raw/source-after.started")
)
assert timestamp(read("raw/source-after.finished")) <= timestamp(
    read("raw/cleanup.started")
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


def usage(kind, budget_bytes, budget_records, backing, records):
    label = "HostVisible" if kind == "HostVisible" else "Device"
    return (
        f"Gfx942{label}BackingUsageV1 {{ budget: Gfx942{label}BackingBudgetV1 {{ "
        f"max_backing_bytes: {budget_bytes}, max_allocations: {budget_records} }}, "
        f"used_backing_bytes: {backing}, used_allocation_records: {records}, "
        f"reserved_records: 0, retained_records: {records}, quarantined_records: 0, poisoned: false }}"
    )


def pool(checked_out, free, capacity, reuse):
    return (
        "Gfx942SdmaMemoryPoolObservationV1 { "
        f"checked_out_buffers: {checked_out}, retained_free_buffers: {free}, "
        f"retained_free_bytes: {capacity}, reuse_count: {reuse} }}"
    )


native_output = read("returned/results/native.stdout")
assert (
    digest("returned/results/native.stdout")
    == "54bc84eafe2fc5975c1781fd265340bf82bca7aaab2979d9407762f4da48e86f"
)
expected_native = (
    f"\nrunning 1 test\ntest {TEST} ... copy_accounting "
    f"allocated_pool={pool(3, 1, 4194272, 64)} "
    f"recycled_pool={pool(0, 4, 809500640, 129)} primary=PrimaryHostUsage {{ "
    f"before: Some({usage('HostVisible', 1073741824, 64, 532480, 3)}), "
    f"completed: Some({usage('HostVisible', 1073741824, 64, 0, 0)}), "
    f"device_before: Some({usage('Device', 268435456, 1, 0, 0)}), "
    f"device_completed: Some({usage('Device', 268435456, 1, 0, 0)}), observations: [1, 1] }}\n"
    "native_runtime_directional_copy_accounting=complete bytes_per_allocation=268435456 "
    "allocations=3 checked_bytes=268435456 roundtrips=1 default_cache=true "
    "host_backing_after=0 device_backing_after=0 external_vram_baseline=not_asserted "
    "performance=not_measured\nok\n\n"
    "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1118 filtered out; "
    "finished in 23.74s\n\n"
)
assert native_output == expected_native
assert (
    read("returned/results/outcome.txt")
    == "native_launched=true native_exit=0 immediate_exit=0 delay_exit=0 delayed_exit=0 inspect_exit=0 no_retry=true\n"
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
    ("post-immediate", [], [298647552] * 3),
    ("post-delayed", [], [298647552] * 3),
):
    assert command("returned/results", name) == observer_command
    rows = [
        load_json(line) for line in read(f"returned/results/{name}.stdout").splitlines()
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
    for snapshot in observation["sysfs"]:
        assert not snapshot["errors"]
        assert snapshot["path"] == "/sys/bus/pci/devices/0000:85:00.0"
        assert snapshot["values"]["unique_id"] == "54f88318ca05093d"
        assert snapshot["values"]["gpu_busy_percent"] == "0"
        assert snapshot["values"]["mem_busy_percent"] == "0"
    for key in ("status", "pids"):
        capture = observation[key]
        assert capture["exit"] == 0 and capture["error"] is None
        assert not capture["stderr"]
        assert timestamp(capture["started"]["utc"]) <= timestamp(
            capture["finished"]["utc"]
        )
    before, between, after = observation["sysfs"]
    status, pids_capture = observation["status"], observation["pids"]
    points = [observation["started"]]
    for step in (before, status, between, pids_capture, after):
        points.extend([step["started"], step["finished"]])
    points.append(observation["finished"])
    assert all(
        earlier["monotonic_ns"] <= later["monotonic_ns"]
        and timestamp(earlier["utc"]) <= timestamp(later["utc"])
        for earlier, later in zip(points, points[1:])
    )
    assert timestamp(read(f"returned/results/{name}.started")) <= timestamp(
        points[0]["utc"]
    )
    assert timestamp(points[-1]["utc"]) <= timestamp(
        read(f"returned/results/{name}.finished")
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
    selected = load_json(observation["status"]["stdout"])["card4"]
    assert selected["Unique ID"] == "0x54f88318ca05093d"
    assert selected["PCI Bus"] == "0000:85:00.0"
    assert selected["GPU use (%)"] == "0"
    assert selected["VRAM Total Used Memory (B)"] == "298647552"
    assert pid_records(observation["pids"]["stdout"]) == {
        3161407: [],
        3161415: [],
        3161403: [0],
        3161411: [],
        3161428: [],
        3161424: [],
        3161432: [],
        3161419: [],
    }
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
    matched = re.findall(r"^([0-9a-f]{64})  (\S+)$", text, re.MULTILINE)
    assert len(matched) == 4
    assert {path: value for value, path in matched} == {
        "runtime-test": BINARY,
        "copy-host-observe.py": OBSERVER,
        "run.sh": digest("run.sh"),
        "inspect.sh": digest("inspect.sh"),
    }
    assert "static-pie linked" in text and "INTERP" not in text
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
    "PASS: one native 256 MiB roundtrip/readback and exact backing-refund test; "
    "11 local and 7 remote receipts; source/binary identity, complete endpoints "
    "and owned cleanup verified; no performance or continuous-isolation claim"
)
