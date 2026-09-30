#!/usr/bin/env python3
"""Strict host-diagnostic decoder; does not grant benchmark acceptance."""

import re

SCHEMA = "fe2o3.xgmi-retained-host-diagnostic.v1"
PROFILE = "fe2o3.gfx942-xgmi-retained-pair-ordinary-lifetime.v1"
POLICY = "18cfe1c56d270d9cab1cdc2f67a2b26b35e7cf1540a2962e4dc2f5cb42155b61"
FIXED = {
    "row": "summary", "schema": SCHEMA, "backend": "kfd", "authority": "none",
    "profile": PROFILE, "policy_sha256": POLICY,
    "environment": "reviewed-mi300x-amdgpu61613-ordinary-lifetime",
    "bytes": "1048576", "warmups": "2", "samples": "10", "prime_batches": "1",
    "direction": "forward-series-then-reverse-series", "wait_policy": "ordinary-unchanged",
    "engine_parallelism": "ordered-single-sdma", "validation": "final-readback",
    "canaries": "pass", "teardown": "explicit", "timing": "instrumented-host-only",
}
SUMMARY_VARIABLE = {
    "unique_ids", "gpu_ids", "depth", "forward_engine", "reverse_engine",
    "forward_entry_ns", "forward_finish_ns", "reverse_entry_ns", "reverse_finish_ns",
}
SUBMIT_PHASES = ("submit_open_ns", "prepare_ns", "publish_ns", "submit_close_ns")
WAIT_PHASES = ("wait_open_ns", "validation_ns", "scan_ns", "retirement_ns", "wait_close_ns")
TIMES = (*SUBMIT_PHASES, "submit_total_ns", *WAIT_PHASES, "wait_total_ns",
         "first_observed_ns", "all_observed_ns")
COUNTERS = ("rounds", "observations", "spins", "yields", "sleeps",
            "requested_sleep_ns", "max_requested_sleep_ns")
CPU = ("thread_cpu_ns", "voluntary_switches", "involuntary_switches")
SAMPLE_KEYS = {"row", "direction", "index", "elapsed_ns", "counters_status", "cpu_status",
               *TIMES, *COUNTERS, *CPU}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def number(value, *, optional=False):
    if optional and value == "none":
        return None
    need(bool(re.fullmatch(r"0|[1-9][0-9]{0,19}", value)), "canonical unsigned integer")
    parsed = int(value)
    need(parsed <= (1 << 64) - 1, "unsigned integer range")
    return parsed


def row(line):
    need(bool(line) and line == line.strip() and "  " not in line, "canonical diagnostic row")
    result = {}
    for token in line.split(" "):
        need(token.count("=") == 1, "key/value token")
        key, value = token.split("=")
        need(bool(re.fullmatch(r"[a-z][a-z0-9_]*", key)) and bool(value) and key not in result,
             "unique diagnostic key")
        result[key] = value
    return result


def parse(raw, expected):
    """Decode one completed invocation against independently admitted endpoints.

    Times are host observations with instrumentation, not device durations.
    This result intentionally has no ordinary performance-acceptance field.
    """
    need(type(raw) is bytes and 0 < len(raw) <= 131072, "bounded raw diagnostic bytes")
    need(set(expected) == {"unique_ids", "gpu_ids", "engines", "depth"}, "exact expected controls")
    need(type(expected["depth"]) is int and expected["depth"] in (1, 16, 32), "closed diagnostic depth")
    for key in ("unique_ids", "gpu_ids", "engines"):
        need(type(expected[key]) in (list, tuple) and len(expected[key]) == 2, "endpoint pair")
    need(all(type(value) is str and re.fullmatch(r"[0-9a-f]{16}", value)
             for value in expected["unique_ids"]), "canonical expected unique IDs")
    need(len(set(expected["unique_ids"])) == 2, "distinct expected unique IDs")
    need(all(type(value) is int and 0 < value < 1 << 32 for value in expected["gpu_ids"])
         and len(set(expected["gpu_ids"])) == 2, "distinct expected KFD GPU IDs")
    need(all(type(value) is int and 2 <= value < 16 for value in expected["engines"]), "XGMI engines")
    text = raw.decode("ascii")
    need(text.endswith("\n") and all(char == "\n" or " " <= char <= "~" for char in text),
         "ASCII diagnostic stream")
    lines = text.splitlines()
    need(len(lines) == 21, "one summary and exactly twenty samples")
    summary = row(lines[0])
    need(set(summary) == set(FIXED) | SUMMARY_VARIABLE, "exact summary keys")
    need(all(summary[key] == value for key, value in FIXED.items()), "exact diagnostic profile")
    need(summary["unique_ids"] == ",".join(expected["unique_ids"]), "admitted unique IDs")
    need(summary["gpu_ids"] == ",".join(map(str, expected["gpu_ids"])), "admitted GPU IDs")
    need(number(summary["depth"]) == expected["depth"], "planned depth")
    for direction, engine in zip(("forward", "reverse"), expected["engines"]):
        need(number(summary[direction + "_engine"]) == engine, "admitted directional engine")
        for boundary in ("entry", "finish"):
            need(number(summary[direction + "_" + boundary + "_ns"]) > 0, "positive scope duration")
    samples = []
    for position, line in enumerate(lines[1:]):
        parsed = row(line)
        need(set(parsed) == SAMPLE_KEYS and parsed["row"] == "sample", "exact sample keys")
        need(parsed["direction"] == ("forward" if position < 10 else "reverse")
             and number(parsed["index"]) == position % 10, "exact ordered sample roster")
        converted = {key: number(parsed[key], optional=True) for key in (*TIMES, *COUNTERS, *CPU)}
        converted.update(direction=parsed["direction"], index=position % 10,
                         elapsed_ns=number(parsed["elapsed_ns"]),
                         counters_status=parsed["counters_status"], cpu_status=parsed["cpu_status"])
        need(converted["elapsed_ns"] > 0, "positive instrumented elapsed time")
        for phases, total in ((SUBMIT_PHASES, "submit_total_ns"), (WAIT_PHASES, "wait_total_ns")):
            values = [converted[key] for key in (*phases, total)]
            if all(value is not None for value in values):
                need(sum(values[:-1]) <= values[-1] <= converted["elapsed_ns"], "nested host phases")
        totals = [converted[key] for key in ("submit_total_ns", "wait_total_ns")]
        if all(value is not None for value in totals):
            need(sum(totals) <= converted["elapsed_ns"], "sequential host call totals")
        offsets = [converted[key] for key in ("first_observed_ns", "all_observed_ns", "scan_ns")]
        if all(value is not None for value in offsets):
            need(offsets[0] <= offsets[1] <= offsets[2], "ordered host observation offsets")
        if parsed["counters_status"] == "available":
            need(all(converted[key] is not None for key in COUNTERS), "available counters")
            rounds = converted["rounds"]
            need(rounds > 0 and converted["spins"] + converted["yields"] + converted["sleeps"] == rounds - 1,
                 "one pause per incomplete scan")
            need(expected["depth"] <= converted["observations"] <= expected["depth"] * rounds,
                 "pending-slot observation bounds")
            pauses = rounds - 1
            need(converted["spins"] == min(pauses, 64)
                 and converted["yields"] == min(max(pauses - 64, 0), 16)
                 and converted["sleeps"] == max(pauses - 80, 0), "unchanged adaptive policy")
            sleeps, requested, maximum = (converted[key] for key in
                                         ("sleeps", "requested_sleep_ns", "max_requested_sleep_ns"))
            need((sleeps == 0 and requested == maximum == 0)
                 or (sleeps > 0 and converted["spins"] == 64 and converted["yields"] == 16
                     and 0 <= maximum <= 1_000_000 and maximum <= requested <= maximum * sleeps),
                 "requested sleep bounds")
        else:
            need(parsed["counters_status"] == "invalid" and all(converted[key] is None for key in COUNTERS),
                 "invalid counter status is not zero")
        if parsed["cpu_status"] == "available":
            need(all(converted[key] is not None for key in CPU), "available CPU observation")
        else:
            need(parsed["cpu_status"] in ("unavailable", "invalid") and all(converted[key] is None for key in CPU),
                 "unavailable CPU observation is not zero")
        samples.append(converted)
    return {"schema": SCHEMA, "authority": "none", "summary": summary, "samples": samples}
