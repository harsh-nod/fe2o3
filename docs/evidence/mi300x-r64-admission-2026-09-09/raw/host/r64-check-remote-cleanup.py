"""Read-only post-run checks on the authorized shared MI300X host."""
import json
import pathlib
import subprocess

paths = [
    "/dev/shm/fe2o3-r64-owner.m0FdGlkA",
    "/dev/shm/fe2o3-r64-owner.NENnw7p5",
    "/proc/2662753", "/proc/2663128", "/proc/2651377", "/proc/2653117",
]
assert not any(pathlib.Path(path).exists() for path in paths), "task path/PID still present"
telemetry = json.loads(subprocess.check_output([
    "/opt/rocm/bin/rocm-smi", "--showuniqueid", "--showbus", "--showuse", "--showmemuse", "--json"
]))
selected = telemetry["card1"]
assert selected["Unique ID"] == "0xab83d2ffef0d3cdf"
assert selected["PCI Bus"].lower() == "0000:26:00.0"
assert int(selected["GPU Memory Allocated (VRAM%)"]) == 0
assert int(selected["GPU use (%)"]) <= 5
shared_crate = pathlib.Path("/home/harsh/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f/futures-executor-0.3.34.crate")
print(json.dumps({"absent_task_paths": paths, "telemetry": telemetry,
    "shared_executor_crate_present": shared_crate.exists()}, sort_keys=True))
