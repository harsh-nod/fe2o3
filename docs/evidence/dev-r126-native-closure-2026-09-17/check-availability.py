#!/usr/bin/env python3
"""Fail closed on unexpected ROCm observations before using physical GPU 1."""

import json
from pathlib import Path
import re
import sys


def require(condition, message):
    if not condition:
        raise SystemExit(message)


usage = json.loads(Path(sys.argv[1]).read_text())
identities = json.loads(Path(sys.argv[2]).read_text())
lines = Path(sys.argv[3]).read_text().splitlines()
require(identities["card1"]["Unique ID"].lower() == "0xab83d2ffef0d3cdf",
        "physical GPU 1 identity changed")
require(identities["card1"]["PCI Bus"] == "0000:26:00.0", "physical GPU 1 bus changed")
require(int(usage["card1"]["GPU use (%)"]) == 0, "GPU 1 is executing work")
used = int(usage["card1"]["VRAM Total Used Memory (B)"])
require(0 <= used <= 512 * 1024 * 1024, "GPU 1 has nonbaseline VRAM occupancy")
pending = None
seen = set()
for raw in lines:
    line = raw.strip()
    if not line or line.startswith("="):
        continue
    if pending is not None:
        require(bool(re.fullmatch(r"\d+(?:\s+\d+)*", line)), "malformed PID device roster")
        devices = [int(value) for value in line.split()]
        require(len(devices) == pending and len(set(devices)) == pending,
                "PID device count mismatch")
        require(1 not in devices, "another process owns GPU 1")
        pending = None
        continue
    match = re.fullmatch(r"PID (\d+) is using (\d+) DRM device\(s\)(:)?", line)
    require(match is not None, "unexpected PID observation")
    pid, count = int(match[1]), int(match[2])
    require(pid > 0 and pid not in seen and (match[3] is not None) == (count > 0),
            "invalid PID observation")
    seen.add(pid)
    pending = count or None
require(pending is None and seen, "incomplete or empty PID observation")
print(f"GPU 1 identity matches; no mapped workload; VRAM={used}; observation only, not a reservation")
