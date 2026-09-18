#!/usr/bin/env python3
"""Complete, read-only endpoint evidence; never an exclusive GPU reservation."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

SCHEMA = "fe2o3.copy-host-observation.v1"
VRAM_LIMIT = 512 * 1024 * 1024
MAX_OUTPUT = 1024 * 1024
VISIBILITY = (
    "HIP_VISIBLE_DEVICES",
    "ROCR_VISIBLE_DEVICES",
    "CUDA_VISIBLE_DEVICES",
    "GPU_DEVICE_ORDINAL",
)
METRICS = (
    "unique_id",
    "mem_info_vram_used",
    "mem_info_vis_vram_used",
    "mem_info_gtt_used",
    "gpu_busy_percent",
    "mem_busy_percent",
)


def stamp():
    now = time.time_ns()
    return {
        "utc": time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(now // 10**9))
        + f".{now % 10**9:09d}Z",
        "monotonic_ns": time.monotonic_ns(),
    }


def decimal(value, maximum):
    if not isinstance(value, str) or re.fullmatch(r"0|[1-9][0-9]*", value) is None:
        raise ValueError("noncanonical decimal")
    number = int(value)
    if number > maximum:
        raise ValueError("decimal exceeds bound")
    return number


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON key")
        result[key] = value
    return result


def parse_status(text, gpu, bdf, uid):
    document = json.loads(text, object_pairs_hook=unique_object)
    card = document[f"card{gpu}"]
    if card["Unique ID"].lower() != uid or card["PCI Bus"].lower() != bdf:
        raise ValueError("SMI GPU identity mismatch")
    return {
        "busy_percent": decimal(card["GPU use (%)"], 100),
        "vram_bytes": decimal(card["VRAM Total Used Memory (B)"], (1 << 64) - 1),
    }


def parse_pids(text):
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    if (
        len(lines) < 4
        or re.fullmatch(r"=+ ROCm System Management Interface =+", lines[0]) is None
        or re.fullmatch(r"=+ GPUs Indexed by PID =+", lines[1]) is None
        or re.fullmatch(r"=+", lines[-2]) is None
        or re.fullmatch(r"=+ End of ROCm SMI Log =+", lines[-1]) is None
    ):
        raise ValueError("incomplete PID transcript")
    rows, pids, index = lines[2:-2], {}, 0
    while index < len(rows):
        match = re.fullmatch(
            r"PID ([1-9][0-9]*) is using ([0-9]+) DRM device\(s\)(:?)", rows[index]
        )
        if not match:
            raise ValueError("unrecognized PID record")
        pid = decimal(match[1], (1 << 31) - 1)
        count = decimal(match[2], 64)
        if pid in pids or bool(count) != bool(match[3]):
            raise ValueError("duplicate PID or invalid GPU count delimiter")
        index += 1
        devices = []
        if count:
            if index == len(rows):
                raise ValueError("missing GPU list")
            devices = [decimal(value, 63) for value in rows[index].split()]
            index += 1
            if len(devices) != count or len(set(devices)) != count:
                raise ValueError("GPU count mismatch or duplicate GPU")
        pids[pid] = devices
    if not pids:
        raise ValueError("no explicit PID records; empty output cannot prove absence")
    return pids


def capture_command(command):
    started = stamp()
    environment = {k: v for k, v in os.environ.items() if k not in VISIBILITY}
    result = {"command": command, "started": started, "exit": None, "error": None}
    try:
        completed = subprocess.run(
            command, capture_output=True, timeout=30, env=environment
        )
        stdout, stderr = completed.stdout, completed.stderr
        result["exit"] = completed.returncode
    except subprocess.TimeoutExpired as error:
        stdout, stderr = error.stdout or b"", error.stderr or b""
        result["error"] = "capture-timeout"
    except OSError as error:
        stdout, stderr = b"", b""
        result["error"] = str(error)
    if len(stdout) > MAX_OUTPUT or len(stderr) > MAX_OUTPUT:
        result["error"] = "oversized-output-truncated"
    for name, data in (("stdout", stdout), ("stderr", stderr)):
        try:
            result[name] = data[:MAX_OUTPUT].decode("ascii")
        except UnicodeDecodeError:
            result[name] = data[:MAX_OUTPUT].decode("ascii", errors="backslashreplace")
            result["error"] = "non-ASCII-output"
    result["finished"] = stamp()
    return result


def capture_sysfs(root, bdf):
    result = {"started": stamp(), "path": str(root / bdf), "values": {}, "errors": {}}
    for name in METRICS:
        try:
            with (root / bdf / name).open("rb") as source:
                value = source.read(129)
            if len(value) > 128 or not value:
                raise ValueError("empty or oversized metric")
            result["values"][name] = value.decode("ascii").strip()
        except (OSError, ValueError) as error:
            result["errors"][name] = str(error)
    result["finished"] = stamp()
    return result


def observe(
    gpu, bdf, uid, smi, *, root=Path("/sys/bus/pci/devices"), run=capture_command
):
    started = stamp()
    prefix = ["/usr/bin/timeout", "--kill-after=5s", "20s", str(smi)]
    before = capture_sysfs(root, bdf)
    status = run(
        prefix
        + [
            "--showuse",
            "--showmeminfo",
            "vram",
            "--showuniqueid",
            "--showbus",
            "--json",
        ]
    )
    between = capture_sysfs(root, bdf)
    # Collect process attribution even after a failed identity, load or VRAM check.
    pids = run(prefix + ["--showpidgpus"])
    after = capture_sysfs(root, bdf)
    reasons, selected_pids = [], None
    for label, snapshot in (("before", before), ("between", between), ("after", after)):
        try:
            values = snapshot["values"]
            if snapshot["errors"] or values["unique_id"].lower() != uid[2:]:
                raise ValueError("missing metrics or sysfs GPU identity mismatch")
            parsed = {
                name: decimal(
                    values[name], 100 if name.endswith("percent") else (1 << 64) - 1
                )
                for name in METRICS
                if name != "unique_id"
            }
            if parsed["gpu_busy_percent"] != 0:
                reasons.append(f"sysfs-{label}-busy")
            if parsed["mem_info_vram_used"] >= VRAM_LIMIT:
                reasons.append(f"sysfs-{label}-vram")
        except (ValueError, KeyError, TypeError, AttributeError):
            reasons.append(f"sysfs-{label}-invalid")
    for label, captured in (("status", status), ("pids", pids)):
        if captured["exit"] != 0 or captured["error"] or captured["stderr"].strip():
            reasons.append(f"{label}-capture-failed")
            continue
        try:
            if label == "status":
                metrics = parse_status(captured["stdout"], gpu, bdf, uid)
                if metrics["busy_percent"] != 0:
                    reasons.append("smi-busy")
                if metrics["vram_bytes"] >= VRAM_LIMIT:
                    reasons.append("smi-vram")
            else:
                selected_pids = sorted(
                    pid
                    for pid, devices in parse_pids(captured["stdout"]).items()
                    if gpu in devices
                )
                if selected_pids:
                    reasons.append("selected-gpu-attachments")
        except (ValueError, KeyError, TypeError, AttributeError):
            reasons.append(f"{label}-invalid")
    return {
        "schema": SCHEMA,
        "record": "observation",
        "started": started,
        "finished": stamp(),
        "gpu_index": gpu,
        "pci_bdf": bdf,
        "unique_id": uid,
        "vram_limit_exclusive": VRAM_LIMIT,
        "visibility_filters": "removed-for-cli",
        "sysfs": [before, between, after],
        "status": status,
        "pids": pids,
        "selected_pids": selected_pids,
        "endpoint_admitted": not reasons,
        "reasons": reasons,
        "scope": "sequential-endpoint-observations-not-continuous-monitoring-or-reservation",
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gpu-index", type=int, required=True)
    parser.add_argument("--pci-bdf", required=True)
    parser.add_argument("--unique-id", required=True)
    parser.add_argument("--samples", type=int, default=1)
    parser.add_argument("--interval-ms", type=int, default=250)
    parser.add_argument("--rocm-smi", type=Path, default=Path("/opt/rocm/bin/rocm-smi"))
    args = parser.parse_args(argv)
    bdf, uid = args.pci_bdf.lower(), args.unique_id.lower()
    if (
        not 0 <= args.gpu_index <= 63
        or not 1 <= args.samples <= 30
        or not 100 <= args.interval_ms <= 10000
        or not args.rocm_smi.is_absolute()
        or re.fullmatch(r"[0-9a-f]{4}:[0-9a-f]{2}:[01][0-9a-f]\.[0-7]", bdf) is None
        or re.fullmatch(r"0x[0-9a-f]{16}", uid) is None
        or int(uid, 16) == 0
    ):
        parser.error("invalid GPU identity, sample bounds or absolute SMI path")
    refused = 0
    for index in range(args.samples):
        observation = observe(args.gpu_index, bdf, uid, args.rocm_smi)
        observation["index"] = index
        print(json.dumps(observation, sort_keys=True), flush=True)
        refused += not observation["endpoint_admitted"]
        if index + 1 < args.samples:
            time.sleep(args.interval_ms / 1000)
    print(
        json.dumps(
            {
                "schema": SCHEMA,
                "record": "complete",
                "observations": args.samples,
                "refused": refused,
                "all_endpoints_admitted": refused == 0,
                "performance_accepted": False,
            },
            sort_keys=True,
        ),
        flush=True,
    )
    return int(refused != 0)


if __name__ == "__main__":
    sys.exit(main())
