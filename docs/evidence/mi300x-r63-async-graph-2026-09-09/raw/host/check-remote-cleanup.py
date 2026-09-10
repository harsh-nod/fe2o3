"""Read-only post-run checks on the authorized shared MI300X host."""
import json
import pathlib
import subprocess

paths = ["/dev/shm/fe2o3-r63-owner.eeLI11k2", "/proc/2480180", "/proc/2480244"]
assert not any(pathlib.Path(path).exists() for path in paths), "task path/PID still present"
telemetry = json.loads(subprocess.check_output([
    "/opt/rocm/bin/rocm-smi", "--showuniqueid", "--showbus", "--showuse", "--showmemuse", "--json"
]))
selected = telemetry["card1"]
assert selected["Unique ID"] == "0xab83d2ffef0d3cdf"
assert selected["PCI Bus"].lower() == "0000:26:00.0"
assert int(selected["GPU Memory Allocated (VRAM%)"]) == 0
assert int(selected["GPU use (%)"]) <= 5
print(json.dumps({"absent_task_paths": paths, "telemetry": telemetry}, sort_keys=True))
