#!/usr/bin/env python3
"""Read-only exact topology and effective CPU/NUMA placement check."""

import json
import os
from pathlib import Path
import subprocess

from protocol import BDF, UID

root = Path("/sys/bus/pci/devices") / BDF
row = {
    name: (root / name).read_text().strip()
    for name in ("unique_id", "numa_node", "local_cpulist")
}
row["affinity"] = sorted(os.sched_getaffinity(0))
placement = subprocess.run(
    ["/usr/bin/numactl", "--show"], capture_output=True, text=True, timeout=5
)
row["placement_stdout"] = placement.stdout
row["placement_stderr"] = placement.stderr
row["placement_exit"] = placement.returncode
print(json.dumps(row, sort_keys=True), flush=True)
assert row["unique_id"].lower() == UID[2:]
assert row["numa_node"] == "1" and row["local_cpulist"] == "48-95"
assert row["affinity"] == list(range(48, 96))
assert placement.returncode == 0 and placement.stderr == ""
assert [line.rstrip() for line in placement.stdout.splitlines()] == [
    "policy: bind",
    "preferred node: 1",
    "physcpubind: " + " ".join(map(str, range(48, 96))),
    "cpubind: 1",
    "nodebind: 1",
    "membind: 1",
    "preferred: 1",
]
