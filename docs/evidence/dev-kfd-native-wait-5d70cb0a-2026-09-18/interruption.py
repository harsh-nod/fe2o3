#!/usr/bin/env python3
"""Validate this closed A/1 interruption without reporting timing ratios."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
stage = Path(__file__).resolve().parent
prior = Path("/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917/docs/evidence/dev-kfd-native-wait-mi300x-2026-09-18/summarize.py")
assert hashlib.sha256(prior.read_bytes()).hexdigest() == "b8fd9dac4a81974d3cc2e13c542cf9a3b004aa41c3eb6f9d7d77e9cd5cb693f9"
spec = importlib.util.spec_from_file_location("pinned_native_wait_protocol", prior)
protocol = importlib.util.module_from_spec(spec)
spec.loader.exec_module(protocol)
owned = "/tmp/fe2o3-kfd-native-wait-5d70cb0a-20260918.CgOcvGXS"
commit = "5d70cb0a6e16fb265fe224690274fdb0be2b0055"
old_binaries = protocol.BINARIES
protocol.BINARIES = tuple(path.replace(protocol.OWNED, owned) for path in old_binaries)
protocol.RUNNER_METADATA -= {path + ": OK" for path in old_binaries}
protocol.RUNNER_METADATA |= {path + ": OK" for path in protocol.BINARIES}
protocol.CONTEXT = dict(protocol.CONTEXT, git_commit=commit)
assert (stage / "raw/benchmark.exit").read_text() == "1\n"
lines = (stage / "raw/benchmark.log").read_text().splitlines()

def one(prefix):
    indices = [i for i, line in enumerate(lines) if line.startswith(prefix)]
    assert len(indices) == 1, (prefix, indices)
    return indices[0]

context, phase, completed, postflight, finished = [one(prefix) for prefix in ("context ", "phase ", "completed ", "postflight ", "finished ")]
assert protocol.fields(lines[context]) == protocol.CONTEXT
assert lines[phase] == "phase repetition=1 cell=A"
assert lines[completed] == "completed repetition=1 cell=A exit=0"
assert lines[postflight] == "postflight repetition=1 cell=A exit=1"
assert lines[finished] == "finished exit=1 source_after_exit=0 binaries_after_exit=0 clean_exit=0 occupancy_exit=0"
cards = [i for i, line in enumerate(lines) if line.startswith('{"card')]
admitted = [i for i, line in enumerate(lines) if line.startswith("admitted ")]
assert len(cards) == 3 and len(admitted) == 2
assert context < phase < cards[0] < admitted[0] < completed < cards[1] < postflight < cards[2] < admitted[1] < finished
covered = {context, phase, completed, postflight, finished, cards[1]}
for start, end in ((cards[0], admitted[0]), (cards[2], admitted[1])):
    protocol.base.validate_guards("\n".join(lines[start:end + 1]), 1)
    covered.update(range(start, end + 1))
payload = lines[admitted[0] + 1:completed]
protocol.base.validate_kfd(payload, "A")
covered.update(range(admitted[0] + 1, completed))
assert len(payload) == 15
assert cards[1] == completed + 2 and postflight == completed + 3
observations = [json.loads(lines[i], object_pairs_hook=protocol.base.unique_object)["card4"] for i in cards]
assert [int(row["VRAM Total Used Memory (B)"]) for row in observations] == [298647552, 648634368, 298754048]
for row in observations:
    assert row["Unique ID"] == "0x54f88318ca05093d"
    assert row["PCI Bus"] == "0000:85:00.0"
    assert row["GPU use (%)"] == "0"
for i, line in enumerate(lines):
    assert i in covered or protocol.runner_metadata(line), (i, line)
assert lines.count("post_run_porcelain=") == 1
assert all(lines.count(binary + ": OK") == 2 for binary in protocol.BINARIES)
timestamps = [lines[index - 1] for index in cards]
assert all(re.fullmatch(protocol.TIMESTAMP, value) for value in timestamps)
assert timestamps == sorted(timestamps)
print(json.dumps({
    "source_commit": commit,
    "campaign_exit": 1,
    "completed_processes": [{"repetition": 1, "cell": "A", "exit": 0, "validated_rounds": 13, "measured_rounds": 10, "checked_bytes_per_round": 268435456, "teardown": "explicit-complete"}],
    "not_launched": ["B", "C", "D"],
    "gpu4": [{"timestamp": timestamp, "utilization_percent": 0, "vram_bytes": int(row["VRAM Total Used Memory (B)"])} for timestamp, row in zip(timestamps, observations)],
    "failed_postflight": "VRAM exceeded 536870912 bytes; guard returned before collecting PID attachments",
    "final_guard": "passed with no reported GPU 4 attachment; does not override prior failure",
    "cause": "unexplained transient usage; no causal or execution-overlap inference",
    "matched_comparison": False,
    "timing_ratios_reported": False,
    "source_after_exit": 0,
    "binaries_after_exit": 0,
    "exact_source_roster_exit": 0
}, indent=2))
