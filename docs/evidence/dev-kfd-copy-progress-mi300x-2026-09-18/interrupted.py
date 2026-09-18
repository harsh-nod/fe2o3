#!/usr/bin/env python3
"""Validate only the observed prefix of this failed campaign, never accept it."""

from datetime import datetime
import json
from pathlib import Path
import re
import types

ARCHIVE = Path(__file__).resolve().parent
summary = types.ModuleType("summary")
summary.__file__ = str(ARCHIVE / "summarize.py")
exec(
    compile(Path(summary.__file__).read_bytes(), summary.__file__, "exec"),
    summary.__dict__,
)
OWNED = "/tmp/fe2o3-kfd-copy-progress-20260918.rrF6He9N"
BINARIES = (
    OWNED + "/target/release/examples/gfx942-runtime-directional-window-benchmark",
    OWNED + "/hsa-copy-pool-engine",
)
STOP = "phase repetition=3 cell=C\n"


def parse(text):
    assert text.count(STOP) == 1
    prefix, stopped = text.split(STOP)
    summary.validate_guards(prefix, 16)
    assert [
        summary.fields(line)
        for line in prefix.splitlines()
        if line.startswith("context ")
    ] == [summary.CONTEXT]
    phases = list(
        re.finditer(r"^phase repetition=([0-9]+) cell=([A-D])\n", prefix, re.M)
    )
    assert [(match[1], match[2]) for match in phases] == summary.EXPECTED[:8]
    header = prefix[: phases[0].start()]
    assert not any(
        line.startswith(
            ("schema=", "admitted ", "completed ", "postflight ", "finished ")
        )
        for line in header.splitlines()
    )
    rows = []
    for i, phase in enumerate(phases):
        end = phases[i + 1].start() if i + 1 < len(phases) else len(prefix)
        lines = prefix[phase.end() : end].splitlines()
        rep, cell = phase[1], phase[2]
        summary.validate_guards("\n".join(lines), 2)
        admissions = [j for j, line in enumerate(lines) if line.startswith("admitted ")]
        completed = f"completed repetition={rep} cell={cell} exit=0"
        postflight = f"postflight repetition={rep} cell={cell} exit=0"
        assert lines.count(completed) == 1 and lines[-1] == postflight
        completion = lines.index(completed)
        assert admissions[0] < completion < admissions[1] == len(lines) - 2
        assert datetime.fromisoformat(lines[0]) <= datetime.fromisoformat(
            lines[completion + 1]
        )
        for guard in (
            lines[: admissions[0] + 1],
            lines[completion + 1 : admissions[1] + 1],
        ):
            assert guard[1].startswith('{"card')
            assert all(
                not line.startswith(
                    ("schema=", "completed ", "postflight ", "phase ", "finished ")
                )
                for line in guard
            )
        payload = lines[admissions[0] + 1 : completion]
        assert payload and all(line.startswith("schema=") for line in payload)
        row = (
            summary.validate_kfd(payload, cell)
            if cell in ("A", "B")
            else summary.validate_hsa(payload, cell)
        )
        rows.append({"repetition": rep, "cell": cell, **row})
    tail = stopped.splitlines()
    assert len(tail) == 9
    for index in (0, 5, 7):
        datetime.fromisoformat(tail[index])
    assert tail[0] <= tail[5] <= tail[7]
    observations = []
    for index in (1, 6):
        observation = json.loads(tail[index], object_pairs_hook=summary.unique_object)[
            "card4"
        ]
        summary.require(
            observation, {"Unique ID": "0x" + summary.UID, "PCI Bus": "0000:85:00.0"}
        )
        assert summary.number(observation, "VRAM Total Used Memory (B)") >= 536870912
        observations.append(observation)
    assert tail[2:5] == [binary + ": OK" for binary in BINARIES] + [
        "post_run_porcelain="
    ]
    assert (
        tail[8]
        == "finished exit=1 source_after_exit=0 binaries_after_exit=0 clean_exit=0 occupancy_exit=1"
    )
    return {
        "scope": "INTERRUPTED campaign; individual prefix observations only, no accepted performance result",
        "accepted_campaign": False,
        "planned_processes": 16,
        "completed_processes": 8,
        "completed_blocks": 2,
        "not_launched_processes": 8,
        "successful_prefix_guard_observations": 16,
        "failed_admission_cell": {"repetition": "3", "cell": "C"},
        "failed_guard_observations": observations,
        "rows": rows,
    }


def main():
    assert (ARCHIVE / "raw/benchmark.exit").read_text() == "1\n"
    assert (ARCHIVE / "raw/benchmark.finished").is_file()
    print(json.dumps(parse((ARCHIVE / "raw/benchmark.log").read_text()), indent=2))


if __name__ == "__main__":
    main()
