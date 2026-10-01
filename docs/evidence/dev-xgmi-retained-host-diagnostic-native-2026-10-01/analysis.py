#!/usr/bin/env python3
"""Offline native24 byte audit and descriptive analysis; never launches work."""

import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import statistics
import sys

COMMIT = "a26dbebb57f948439a2e813c7a17c1142014ba1e"
BINDING = "b4d89265c8861e3bcf39517e4c96cd2a7885e0753f688051451082eb0bf38e82"
ARCHIVE = "1375a5d7dd8968bd905a0a1ce74add0b3a2c38bf5336f44f63dc28a0592f2227"
OWNER = "/home/harsh/fe2o3-xgmi-series-20260930.0b26c1f6eb613ac2"
DEPTHS = (1, 16, 32)
DIRECTIONS = ("forward", "reverse")
BACKENDS = ("kfd", "hsa", "hip")


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def load(path):
    return json.loads(path.read_bytes())


def write(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n", encoding="ascii")


def census(folder, value):
    order = value["attempt_order"]
    pins = value["raw_sha256"]
    need(len(order) == len(set(order)) and set(order) == set(pins), "unique census roster")
    need({item.name for item in folder.iterdir()} == set(order), "exact closed command directories")
    rows, pids = [], set()
    for name in sorted(order):
        item = folder / name
        need(item.is_dir() and not item.is_symlink(), "ordinary command folder")
        need({path.name for path in item.iterdir()} == {"stdout", "stderr", "receipt.json"}, "raw stream roster")
        need({key: sha(item / key) for key in pins[name]} == pins[name], "census raw hashes")
        receipt = load(item / "receipt.json")
        pid = receipt["pid"]
        need(type(pid) is int and pid > 0 and pid not in pids, "distinct recorded process occurrence")
        pids.add(pid)
        need(receipt["exit"] == 0 and receipt["error"] is None and receipt["group_absent"] is True,
             "successful original closed group receipt")
        need(receipt["finished_ns"] >= receipt["started_ns"], "ordered receipt timestamps")
        need(all(receipt[key + "_sha256"] == sha(item / key) for key in ("stdout", "stderr")), "raw stream joins")
        rows.append({"name": name, "pid": pid, "receipt_sha256": sha(item / "receipt.json")})
    need(rows == value["records"], "exact retained census rows")
    need(value["scope"] == "fresh-recorder-closed-groups-no-historical-pid-probes", "census scope")
    return {"count": len(rows), "recorded_namespace": value["namespace"],
            "raw_hashes_verified": True, "recorded_groups_closed": True,
            "historical_pid_probes_performed": False}


def distribution(values):
    present = sorted(value for value in values if value is not None)
    need(all(type(value) in (int, float) and value >= 0 for value in present), "nonnegative observations")
    return {"n": len(values), "available": len(present), "missing": len(values) - len(present),
            "min": min(present) if present else None,
            "median": statistics.median(present) if present else None,
            "p95_nearest_rank": present[math.ceil(0.95 * len(present)) - 1] if present else None,
            "max": max(present) if present else None,
            "mean": statistics.mean(present) if present else None}


def sum_available(sample, keys):
    values = [sample[key] for key in keys]
    return sum(values) if all(value is not None for value in values) else None


def analyze(replay, diagnostic):
    ordinary, profiled = [], []
    need(len(replay["ordinary"]["trials"]) == 18 and len(replay["profiled"]) == 6, "exact trial partitions")
    metrics = ("elapsed_ns", *diagnostic.TIMES, *diagnostic.COUNTERS, *diagnostic.CPU)
    need(len(metrics) == len(set(metrics)), "unique metric roster")
    check_keys = ("submit_open_ns", "submit_close_ns", "wait_open_ns", "wait_close_ns")
    derived_keys = {
        "operational_checks_ns": check_keys,
        "submit_named_phases_ns": diagnostic.SUBMIT_PHASES,
        "wait_named_phases_ns": diagnostic.WAIT_PHASES,
        "all_named_phases_ns": (*diagnostic.SUBMIT_PHASES, *diagnostic.WAIT_PHASES),
        "both_call_totals_ns": ("submit_total_ns", "wait_total_ns"),
    }
    all_samples = []
    for depth in DEPTHS:
        for direction in DIRECTIONS:
            cells = {}
            for backend in BACKENDS:
                trials = [row for row in replay["ordinary"]["trials"]
                          if int(row["fields"]["depth"]) == depth and row["fields"]["backend"] == backend]
                need(len(trials) == 2, "two ordinary invocations per cell")
                invocations = []
                for row in trials:
                    fields = row["fields"]
                    value = {"name": row["name"], "p50_ns": int(fields[direction + "_p50_ns"]),
                             "p95_ns": int(fields[direction + "_p95_ns"]),
                             "stdout_sha256": row["stdout_sha256"],
                             "execution_receipt_sha256": row["execution_receipt_sha256"]}
                    if backend == "kfd":
                        value.update(scope_entry_ns=int(fields[direction + "_scope_entry_ns"]),
                                     scope_finish_ns=int(fields[direction + "_scope_finish_ns"]))
                    invocations.append(value)
                cells[backend] = {"invocations": invocations,
                                  "mean_of_two_invocation_p50_ns": statistics.mean(row["p50_ns"] for row in invocations)}
            kfd = cells["kfd"]["mean_of_two_invocation_p50_ns"]
            ordinary.append({"depth": depth, "direction": direction, "backends": cells,
                             "kfd_over_hsa_percent": 100 * (kfd / cells["hsa"]["mean_of_two_invocation_p50_ns"] - 1),
                             "kfd_over_hip_percent": 100 * (kfd / cells["hip"]["mean_of_two_invocation_p50_ns"] - 1)})
            trials = [row for row in replay["profiled"] if int(row["fields"]["summary"]["depth"]) == depth]
            need(len(trials) == 2, "two diagnostic invocations per depth")
            samples, invocations = [], []
            for row in trials:
                current = [sample for sample in row["fields"]["samples"] if sample["direction"] == direction]
                need(len(current) == 10 and [sample["index"] for sample in current] == list(range(10)), "ten ordered samples")
                for sample in current:
                    derived = {key: sum_available(sample, keys) for key, keys in derived_keys.items()}
                    need(all(value is None or value <= sample["elapsed_ns"] for value in derived.values()),
                         "per-sample nonoverlapping sums fit elapsed interval")
                    samples.append({**sample, **derived, "invocation": row["name"]})
                invocations.append({"name": row["name"], "elapsed_ns": distribution([sample["elapsed_ns"] for sample in current]),
                                    "scope_entry_ns": int(row["fields"]["summary"][direction + "_entry_ns"]),
                                    "scope_finish_ns": int(row["fields"]["summary"][direction + "_finish_ns"]),
                                    "stdout_sha256": row["stdout_sha256"],
                                    "execution_receipt_sha256": row["execution_receipt_sha256"]})
            need(len(samples) == 20, "twenty diagnostic samples per direction/depth")
            distributions = {key: distribution([sample[key] for sample in samples]) for key in (*metrics, *derived_keys)}
            histograms = {key: dict(sorted(Counter(str(sample[key]) for sample in samples).items()))
                          for key in (*diagnostic.COUNTERS, "voluntary_switches", "involuntary_switches")}
            profiled.append({"depth": depth, "direction": direction, "invocations": invocations,
                             "distributions": distributions, "histograms": histograms,
                             "cpu_status_counts": dict(Counter(sample["cpu_status"] for sample in samples)),
                             "counters_status_counts": dict(Counter(sample["counters_status"] for sample in samples)),
                             "derived_checks_fraction_of_elapsed": distribution([
                                 sample["operational_checks_ns"] / sample["elapsed_ns"]
                                 if sample["operational_checks_ns"] is not None else None for sample in samples]),
                             "scan_fraction_of_elapsed": distribution([
                                 sample["scan_ns"] / sample["elapsed_ns"]
                                 if sample["scan_ns"] is not None else None for sample in samples])})
            all_samples.extend(samples)
    return {"ordinary": ordinary, "profiled": profiled, "profiled_sample_count": len(all_samples),
            "missing_by_field": {key: sum(sample[key] is None for sample in all_samples) for key in metrics},
            "derived_sum_terms": {key: list(keys) for key, keys in derived_keys.items()},
            "statistical_conventions": {
                "ordinary": "Arithmetic mean of the two recorded invocation p50s; not a pooled raw-sample median.",
                "diagnostic": "Descriptive distributions over twenty instrumented samples per depth/direction, with two invocation distributions retained.",
                "median": "Middle value for odd n, arithmetic mean of two central values for even n.",
                "p95": "Nearest rank: sorted present observations at one-based index ceil(0.95*n).",
                "missing": "None excluded from quantiles and means, counted explicitly; never replaced with zero.",
                "sums": "Named nonoverlapping phases summed within each sample before distribution; never sum marginal medians.",
                "cpu": "Thread CPU and switches bracket only the scan, not the full call or measured batch.",
                "sleep": "Requested sleep is a policy counter, not measured asleep time or avoidable delay.",
            }}


def readme(result):
    rows = ["# Native24 Offline Analysis", "", "Source: `" + COMMIT + "`. Original execution session: `1309`, terminal `0`.",
            "", "This is descriptive evidence from one nonexclusive point-idle MI300X campaign, not performance acceptance, engine equivalence, or formal refinement.",
            "", "## Ordinary Comparison", "", "Mean of two invocation p50s, microseconds per batch. Each copy is 1 MiB; depth is copies per batch.",
            "", "| Depth | Direction | KFD | HSA | HIP | KFD/HSA delta | KFD/HIP delta |",
            "| --- | --- | ---: | ---: | ---: | ---: | ---: |"]
    for cell in result["analysis"]["ordinary"]:
        values = [cell["backends"][key]["mean_of_two_invocation_p50_ns"] / 1000 for key in BACKENDS]
        rows.append(f"| {cell['depth']} | {cell['direction']} | " + " | ".join(f"{value:.3f}" for value in values) +
                    f" | {cell['kfd_over_hsa_percent']:+.2f}% | {cell['kfd_over_hip_percent']:+.2f}% |")
    rows += ["", "## Instrumented Host Distributions", "",
             "Every cell has twenty samples (ten from each of two invocations). Entries are median / nearest-rank p95 in microseconds. Rows are independent distributions and must not be added. Only `operational_checks_ns` and other explicitly named derived fields are per-sample sums.", "",
             "| Depth | Direction | Elapsed | Submit total | Wait total | Four checks | Roster validation | Scan | Scan thread CPU | Retirement |",
             "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for cell in result["analysis"]["profiled"]:
        values = []
        for key in ("elapsed_ns", "submit_total_ns", "wait_total_ns", "operational_checks_ns", "validation_ns", "scan_ns", "thread_cpu_ns", "retirement_ns"):
            metric = cell["distributions"][key]
            values.append("missing" if metric["median"] is None else f"{metric['median']/1000:.3f} / {metric['p95_nearest_rank']/1000:.3f}")
        rows.append(f"| {cell['depth']} | {cell['direction']} | " + " | ".join(values) + " |")
    rows += ["", "Complete min/median/p95/max/mean and missing counts for every phase, observation offset, CPU field, and counter are in `analysis.json`. Spin/yield/sleep and context-switch histograms are retained there, as are both invocation identities.",
             "", "Missing observations across 120 profiled samples: " + str(sum(result["analysis"]["missing_by_field"].values())) + " field-values.",
             "", "## Limits", "",
             "- Instrumentation changes timing and readiness; profiled data is not an ordinary HIP/HSA comparison.",
             "- Scan wall time includes completion polling, yielding and sleeping while GPU progress overlaps it; it is not pure host overhead.",
             "- Requested sleep is not actual sleep duration. No wall-minus-CPU or requested-sleep value is called avoidable latency.",
             "- Completion offsets are host observations, not device timestamps. HSA/HIP engine identities are unobserved.",
             "- Operational reset/VRAM checks and the wait policy remain unchanged. Scope entry and finish are outside ordinary timed samples.",
             "- Fresh device checks are point-in-time observations, not an exclusive reservation or proof of absent interference.",
             "- Native success and byte replay do not establish universal driver correctness, full HIP/HSA parity, Context-facade coverage, or A7 acceptance.",
             "", "## Reproduction", "", "Run the adjacent Python controller with `python3 -I -B`, `--prepared` pointing to the retained attempt-1 directory, and `--output` pointing to a new empty result directory. It audits raw hashes, source binding, exact archive members, recorded closed groups and independent replay without launching commands, querying GPUs, or probing old PIDs.", ""]
    return "\n".join(rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prepared", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    need(sys.flags.isolated and sys.dont_write_bytecode, "use python3 -I -B")
    prepared = args.prepared.resolve(strict=True)
    need(not args.output.exists(), "fresh output directory required")
    marker = load(prepared / "owner.json")
    need(marker == {"commit": COMMIT, "binding_sha256": BINDING, "path": OWNER}, "exact native24 owner")
    sys.path.insert(0, str(prepared / "source/benchmarks/runtime_gfx942"))
    import xgmi_retained_host_diagnostic_transport as extension
    import xgmi_retained_host_diagnostic as diagnostic
    transport, native, hot, ordinary, experiment, planner = extension.helpers()
    binding = transport.read_binding(prepared, marker)
    extension.extended_binding(prepared, binding, transport)
    need(sha(prepared / "source.tar.gz") == binding["payload_sha256"], "source archive binding")
    transport.validate_archive(prepared / "source.tar.gz", binding["files"])
    collection = load(prepared / "remote-commands/collect/stdout")
    archive = prepared / "remote-commands/pull/stdout"
    need(collection["marker"] == marker and collection["archive_sha256"] == ARCHIVE == sha(archive), "exact collected archive")
    need(collection["archive_bytes"] == archive.stat().st_size, "collected archive size")
    transport.validate_archive(archive, collection["files"])
    readback = prepared / "readback"
    need(transport.inventory(readback) == collection["files"], "exact unchanged extracted collection")
    replay = extension.independent_replay(readback, marker, binding, transport, hot, ordinary, experiment, planner)
    need(replay == load(prepared / "independent-replay.json") and replay["accepted"] is True, "independent native24 replay")
    finished = load(prepared / "transport-finished.json")
    need(finished["accepted"] is True and finished["original_remote_terminal"] is True and not finished["failures"], "original terminal acceptance")
    need(load(prepared / "remote-commands/cleanup/stdout") == {"removed": OWNER, "archive_sha256": ARCHIVE}, "exact owned cleanup")
    need(load(prepared / "remote-commands/absence/stdout") == {"path_absent": OWNER}, "recorded exact absence")
    censuses = {
        "prepare": census(prepared / "commands", load(prepared / "prepare-census.json")),
        "transport": census(prepared / "remote-commands", load(prepared / "transport-census.json")),
        "native": census(readback / "campaign-1/commands", load(readback / "campaign-1/fresh-census.json")),
    }
    need([censuses[key]["count"] for key in ("prepare", "transport", "native")] == [14, 6, 327], "closed stage counts")
    paths = ("owner.json", "binding.json", "prepare-census.json", "transport-census.json", "transport-finished.json", "independent-replay.json",
             "remote-commands/collect/stdout", "remote-commands/pull/stdout", "remote-commands/cleanup/receipt.json", "remote-commands/absence/receipt.json",
             "readback/launch.json", "readback/monitor/receipt.json", "readback/campaign-1/finished.json", "readback/campaign-1/replay.json",
             "readback/campaign-1/fresh-census.json", "readback/campaign-1/binaries-before.json", "readback/campaign-1/binaries-after.json")
    result = {"schema": "fe2o3.native24-offline-descriptive-analysis.v1", "source_commit": COMMIT,
              "analysis_controller_sha256": sha(Path(__file__).resolve()), "original_session": 1309,
              "marker": marker, "independent_replay": replay, "censuses": censuses,
              "input_sha256": {name: sha(prepared / name) for name in paths},
              "archive": {"sha256": ARCHIVE, "bytes": collection["archive_bytes"], "files": len(collection["files"]),
                          "uncompressed_bytes": sum(row["bytes"] for row in collection["files"].values())},
              "binaries_sha256": load(readback / "campaign-1/binaries-before.json"),
              "analysis": analyze(load(readback / "campaign-1/replay.json"), diagnostic),
              "performance_acceptance": False, "formal_refinement": False, "engine_matching": False,
              "exclusive_reservation": False, "new_gpu_execution": False, "historical_pid_probes": False}
    args.output.mkdir(mode=0o700)
    write(args.output / "analysis.json", result)
    (args.output / "README.md").write_text(readme(result), encoding="ascii")
    print(json.dumps({"output": str(args.output), "analysis_sha256": sha(args.output / "analysis.json"),
                      "readme_sha256": sha(args.output / "README.md"), "closed_stage_counts": [14, 6, 327],
                      "profiled_samples": result["analysis"]["profiled_sample_count"]}, sort_keys=True))


if __name__ == "__main__":
    main()
