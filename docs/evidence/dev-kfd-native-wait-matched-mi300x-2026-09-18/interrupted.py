#!/usr/bin/env python3
"""Verify the closed A/1 interruption without publishing performance metrics."""

import importlib.util
import json
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
ARCHIVE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location(
    "native_summary", ARCHIVE / "summarize.py"
)
wrapper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wrapper)
summary = wrapper.protocol


def parse_pids(lines):
    result, index = {}, 0
    while index < len(lines):
        line = lines[index]
        index += 1
        if not line.strip() or re.fullmatch(r"={20,}(?:[^=]+={20,})?", line):
            continue
        row = re.fullmatch(r"PID ([0-9]+) is using ([0-9]+) DRM device\(s\)(:)?", line)
        assert row, line
        pid, count = int(row[1]), int(row[2])
        assert pid not in result and 0 <= count <= 8
        assert bool(row[3]) == (count > 0)
        devices = []
        if count:
            devices = lines[index].split()
            index += 1
            assert len(devices) == count and all(
                re.fullmatch(r"[0-7]", d) for d in devices
            )
        result[pid] = set(map(int, devices))
        assert len(result[pid]) == count
    assert result
    return result


def parse(text):
    lines = text.splitlines()
    protocol = [
        line
        for line in lines
        if line.startswith(
            (
                "context ",
                "phase ",
                "admitted ",
                "completed ",
                "postflight ",
                "finished ",
            )
        )
    ]
    assert len(protocol) == 6
    assert summary.fields(protocol[0]) == summary.CONTEXT
    assert protocol[1:] == [
        "phase repetition=1 cell=A",
        "admitted gpu=4 uid=0x54f88318ca05093d bdf=0000:85:00.0",
        "completed repetition=1 cell=A exit=0",
        "postflight repetition=1 cell=A exit=1",
        "finished exit=1 source_after_exit=0 binaries_after_exit=0 clean_exit=0 occupancy_exit=1",
    ]
    context, phase, admitted, completed, postflight, finished = [
        lines.index(line) for line in protocol
    ]
    assert phase == context + 1 and finished == len(lines) - 1
    assert lines[:context] == [
        *(binary + ": OK" for binary in summary.BINARIES),
        summary.TOPOLOGY,
        "policy: bind",
        "preferred node: 1",
        "physcpubind: " + " ".join(str(i) for i in range(48, 96)) + " ",
        "cpubind: 1 ",
        "nodebind: 1 ",
        "membind: 1 ",
        "preferred: 1 ",
    ]
    assert re.fullmatch(summary.TIMESTAMP, lines[phase + 1])
    summary.base.validate_guards("\n".join(lines[phase + 2 : admitted + 1]), 1)
    legacy = summary.base.validate_kfd(lines[admitted + 1 : completed], "A")
    assert completed + 3 < postflight and finished == postflight + 7
    attached = parse_pids(lines[completed + 3 : postflight])
    assert attached[536718] == set(range(8))
    assert lines[postflight + 1 : postflight + 4] == [
        *(binary + ": OK" for binary in summary.BINARIES),
        "post_run_porcelain=",
    ]
    stamp_indices = (phase + 1, completed + 1, postflight + 4, finished - 1)
    stamps = [lines[index] for index in stamp_indices]
    assert all(re.fullmatch(summary.TIMESTAMP, stamp) for stamp in stamps)
    assert stamps == sorted(set(stamps))
    snapshot_indices = (phase + 2, completed + 2, postflight + 5)
    assert [i for i, line in enumerate(lines) if line.startswith('{"card')] == list(
        snapshot_indices
    )
    cards = []
    for index, used, utilization in zip(
        snapshot_indices, (298647552, 298659840, 648527872), (0, 0, 0)
    ):
        snapshot = json.loads(
            lines[index], object_pairs_hook=summary.base.unique_object
        )
        assert set(snapshot) == {f"card{i}" for i in range(8)}
        card = snapshot["card4"]
        assert card == {
            "Unique ID": "0x" + summary.base.UID,
            "PCI Bus": "0000:85:00.0",
            "GPU use (%)": str(utilization),
            "VRAM Total Memory (B)": "206141652992",
            "VRAM Total Used Memory (B)": str(used),
        }
        cards.append(card)
    return {
        "source": summary.COMMIT,
        "accepted_campaign": False,
        "completed_processes": ["A/1"],
        "completed_legacy_rounds": len(legacy["rounds"]),
        "performance_qualified_processes": 0,
        "native_profiled_processes": 0,
        "hsa_processes": 0,
        "comparisons": None,
        "stop_reason": "A/1 postflight found a process attached to GPU 4; final VRAM threshold failed",
        "postflight_attached_pid": 536718,
        "postflight_attached_gpu_indices": sorted(attached[536718]),
        "gpu4_snapshots": cards,
        "scope": "Legacy buffer validation and explicit teardown completed; no qualified timings, native-wait hardware coverage, performance attribution, parity, or formal-proof claim. A separate attached process was observed after A/1; whether it overlapped the measured interval is not established.",
    }


if __name__ == "__main__":
    assert (ARCHIVE / "raw/benchmark.exit").read_text() == "1\n"
    print(json.dumps(parse((ARCHIVE / "raw/benchmark.log").read_text()), indent=2))
