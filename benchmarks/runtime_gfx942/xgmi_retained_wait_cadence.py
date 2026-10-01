#!/usr/bin/env python3
"""Strict distinct cadence-experiment decoder; no admission or performance claim."""

import re

import xgmi_retained_host_diagnostic as baseline

SCHEMA = "fe2o3.xgmi-retained-wait-cadence.v1"
CEILINGS = {"ordinary-1ms": 1_000_000, "ceiling-25us": 25_000}
MODES = ("ordinary", "profiled")


def parse(raw, expected):
    need, number, row = baseline.need, baseline.number, baseline.row
    need(type(raw) is bytes and 0 < len(raw) <= 131072, "bounded raw cadence stream")
    need(set(expected) == {"unique_ids", "gpu_ids", "engines", "depth", "cadence", "mode"}, "exact cadence controls")
    need(type(expected["depth"]) is int and expected["depth"] in (1, 16, 32), "closed depth")
    need(type(expected["cadence"]) is str and expected["cadence"] in CEILINGS, "closed cadence")
    need(type(expected["mode"]) is str and expected["mode"] in MODES, "closed instrumentation mode")
    for key in ("unique_ids", "gpu_ids", "engines"):
        need(type(expected[key]) in (list, tuple) and len(expected[key]) == 2, "exact endpoint pair")
    need(all(type(uid) is str and re.fullmatch(r"[0-9a-f]{16}", uid) for uid in expected["unique_ids"])
         and len(set(expected["unique_ids"])) == 2, "canonical distinct unique IDs")
    need(all(type(gpu) is int and 0 < gpu < 1 << 32 for gpu in expected["gpu_ids"])
         and len(set(expected["gpu_ids"])) == 2, "distinct KFD GPU IDs")
    need(all(type(engine) is int and 2 <= engine < 16 for engine in expected["engines"]), "XGMI engines")
    text = raw.decode("ascii")
    need(text.endswith("\n") and all(char == "\n" or " " <= char <= "~" for char in text), "canonical ASCII rows")
    lines = text.splitlines()
    need(len(lines) == 21, "one summary and twenty samples")
    summary = row(lines[0])
    mode, cadence = expected["mode"], expected["cadence"]
    ceiling = CEILINGS[cadence]
    fixed = {**baseline.FIXED, "schema": SCHEMA, "wait_policy": "explicit-cadence-experiment",
             "mode": mode, "cadence": cadence, "sleep_ceiling_ns": str(ceiling),
             "timing": "instrumented-host-only" if mode == "profiled" else "native-enqueue-through-paired-operational-completion"}
    need(set(summary) == set(fixed) | baseline.SUMMARY_VARIABLE, "exact experiment summary keys")
    need(all(summary[key] == value for key, value in fixed.items()), "exact policy and mode binding")
    need(summary["unique_ids"] == ",".join(expected["unique_ids"]), "admitted UID pair")
    need(summary["gpu_ids"] == ",".join(map(str, expected["gpu_ids"])), "admitted GPU pair")
    need(number(summary["depth"]) == expected["depth"], "planned depth")
    for direction, engine in zip(("forward", "reverse"), expected["engines"], strict=True):
        need(number(summary[direction + "_engine"]) == engine, "directional engine")
        for boundary in ("entry", "finish"):
            need(number(summary[direction + "_" + boundary + "_ns"]) > 0, "positive scope duration")
    samples = []
    for position, line in enumerate(lines[1:]):
        fields = row(line)
        keys = baseline.SAMPLE_KEYS if mode == "profiled" else {"row", "direction", "index", "elapsed_ns"}
        need(set(fields) == keys and fields["row"] == "sample", "exact mode-specific sample keys")
        need(fields["direction"] == ("forward" if position < 10 else "reverse")
             and number(fields["index"]) == position % 10, "ordered sample roster")
        sample = {"direction": fields["direction"], "index": position % 10, "elapsed_ns": number(fields["elapsed_ns"])}
        need(sample["elapsed_ns"] > 0, "positive elapsed time")
        if mode == "profiled":
            sample.update({key: number(fields[key], optional=True) for key in (*baseline.TIMES, *baseline.COUNTERS, *baseline.CPU)})
            sample.update(counters_status=fields["counters_status"], cpu_status=fields["cpu_status"])
            for phases, total in ((baseline.SUBMIT_PHASES, "submit_total_ns"), (baseline.WAIT_PHASES, "wait_total_ns")):
                values = [sample[key] for key in (*phases, total)]
                if all(value is not None for value in values):
                    need(sum(values[:-1]) <= values[-1] <= sample["elapsed_ns"], "nested measured phases")
            totals = [sample[key] for key in ("submit_total_ns", "wait_total_ns")]
            if all(value is not None for value in totals):
                need(sum(totals) <= sample["elapsed_ns"], "sequential call totals")
            offsets = [sample[key] for key in ("first_observed_ns", "all_observed_ns", "scan_ns")]
            if all(value is not None for value in offsets):
                need(offsets[0] <= offsets[1] <= offsets[2], "host observation offsets")
            if fields["counters_status"] == "available":
                need(all(sample[key] is not None for key in baseline.COUNTERS), "available counters")
                rounds = sample["rounds"]
                need(rounds > 0 and sample["spins"] + sample["yields"] + sample["sleeps"] == rounds - 1, "pause count")
                need(expected["depth"] <= sample["observations"] <= expected["depth"] * rounds, "pending observations")
                pauses = rounds - 1
                need(sample["spins"] == min(pauses, 64) and sample["yields"] == min(max(pauses - 64, 0), 16)
                     and sample["sleeps"] == max(pauses - 80, 0), "unchanged spin/yield policy")
                sleeps, requested, maximum = (sample[key] for key in ("sleeps", "requested_sleep_ns", "max_requested_sleep_ns"))
                need((sleeps == 0 and requested == maximum == 0)
                     or (sleeps > 0 and 0 <= maximum <= ceiling and maximum <= requested <= maximum * sleeps),
                     "selected requested-sleep ceiling, not actual sleep")
            else:
                need(fields["counters_status"] == "invalid" and all(sample[key] is None for key in baseline.COUNTERS), "invalid counters are not zero")
            if fields["cpu_status"] == "available":
                need(all(sample[key] is not None for key in baseline.CPU), "available CPU observation")
            else:
                need(fields["cpu_status"] in ("unavailable", "invalid") and all(sample[key] is None for key in baseline.CPU), "missing CPU is not zero")
        samples.append(sample)
    return {"schema": SCHEMA, "authority": "none", "summary": summary, "samples": samples,
            "performance_acceptance": False, "formal_refinement": False,
            "instrumentation_perturbs_timing_and_readiness": mode == "profiled"}
