"""Read-only post-run checks on the authorized shared MI300X host."""
import json
import os
import pathlib
import subprocess

paths = [
    "/dev/shm/fe2o3-r65-owner.1Cci4FZq",
    "/proc/3655843", "/proc/3658984", "/proc/3659540", "/proc/3664412",
    "/proc/3665864", "/proc/3666022",
]
assert not any(os.path.lexists(path) for path in paths), "task path/PID still present"
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
