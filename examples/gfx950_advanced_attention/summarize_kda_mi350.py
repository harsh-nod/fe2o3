#!/usr/bin/env python3
"""Summarize replicated fe2o3 and FLA KDA measurements from MI350."""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import statistics
from pathlib import Path
from typing import Any, Callable


FE2O3_VARIANTS = {
    "decode_baseline": "decode-duplicate-key-read",
    "decode_final": "decode-explicit-key-reuse",
    "prefill_baseline": "prefill-sequential-recurrence",
    "prefill_final": "prefill-c4-wy-ut",
}

ISA_EVIDENCE = {
    "decode_duplicate_key_read": {
        "binding_sha256": "160b57240e4d405563c3dd402992eb50ac0b1192c795954e6853d2fe08b4dd09",
        "campaign_hsaco_sha256": "63e87d54598720575e4a6734ae3ca3949e1ba97725e0a6d1b432bbd288d4ae3a",
        "sgprs": 58,
        "vgprs": 14,
        "lds_bytes": 0,
        "private_segment_bytes": 0,
        "global_load_instructions": 5,
        "ds_bpermute_instructions": 8,
        "s_waitcnt_instructions": 14,
    },
    "decode_explicit_key_reuse": {
        "binding_sha256": "e249ff03f475aa75595229ee6a68e816a2a9ad395940c495ad874c54c0e9b0ad",
        "campaign_hsaco_sha256": "9f65a1407247cafa26eebaa574a92037cadc4bd1f11da1c81535927fc9cafde4",
        "sgprs": 58,
        "vgprs": 14,
        "lds_bytes": 0,
        "private_segment_bytes": 0,
        "global_load_instructions": 5,
        "ds_bpermute_instructions": 8,
        "s_waitcnt_instructions": 11,
    },
    "prefill_sequential_recurrence": {
        "binding_sha256": "083c8464c05f4af00df5503e2a5905f65e7b865610f441f9eca4a3c7e556efa6",
        "campaign_hsaco_sha256": "7b17631b97cd431d6d3f4c12c1132ddc8acc4e2165963655992c6bde9df1018c",
        "sgprs": 58,
        "vgprs": 35,
        "lds_bytes": 0,
        "private_segment_bytes": 0,
        "global_load_instructions": 33,
        "global_store_instructions": 3,
        "ds_bpermute_instructions": 64,
        "s_waitcnt_instructions": 92,
    },
    "prefill_c4_wy_ut": {
        "binding_sha256": "673210266e41c1a545820dbc0baec859659b5c1cf4d5e3e8ac6b5e542b4028d3",
        "campaign_hsaco_sha256": "af4b48c379010aca3b5d64761eb92058823d042a0e95b5a7ded0cc81c0ea7791",
        "sgprs": 58,
        "vgprs": 85,
        "lds_bytes": 0,
        "private_segment_bytes": 0,
        "global_load_instructions": 33,
        "global_store_instructions": 3,
        "ds_bpermute_instructions": 192,
        "s_waitcnt_instructions": 135,
    },
}

NEGATIVE_ABLATIONS = {
    "workgroup_64_four_columns_per_lane": {
        "status": "correctness-passed, performance-rejected",
        "protocol": "single process using the same 1,000-warmup, 3,000-dispatch sampling shape",
        "decode_median_ns": 6_160.0,
        "decode_regression_vs_final_percent": 100.0 * (6_160.0 / 4_720.0 - 1.0),
        "decode_raw_sha256": "37e86e7d836ee94a4fd247513a03b12a217ec0e3417d9f6fbdcc4c4110fc5b47",
        "decode_hsaco_sha256": "de74f1be03cee6d88e08c8c15d6db54f2aea6fa3392a8442b15a6d216a09d5f2",
        "prefill_median_ns": 13_160.0,
        "prefill_regression_vs_final_percent": 100.0 * (13_160.0 / 11_400.0 - 1.0),
        "prefill_raw_sha256": "4f7f8024523725262e06d225d4704b56aeab2d2354a97078b94377d14b4163d7",
        "prefill_hsaco_sha256": "42842b56e5bb75cc107406d4e7a49e2bb4d5a550c002295ecbc7d28cefdfe918",
        "interpretation": "column coarsening increased live state and serialized the Wave16 reduction",
    },
    "workgroup_128_pair_tile": {
        "status": "compiler-rejected",
        "contribution_percent": None,
        "interpretation": "the verifier could not prove convergence around the early-exit reduction",
    },
    "subgroup_load_coalescing": {
        "status": "compiler-rejected",
        "contribution_percent": None,
        "interpretation": "conditional producer lanes before subgroup broadcasts were not proven convergent",
    },
    "lds_multibuffering": {
        "status": "not-applicable",
        "contribution_percent": 0.0,
        "interpretation": "the matrix state is register resident and there is no streamed K tile to stage; LDS would add barriers and reloads",
    },
}


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fe2o3-directory", type=Path, required=True)
    parser.add_argument("--fla-directory", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--bootstrap-resamples", type=int, default=20_000)
    return parser.parse_args()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    records = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    if not records:
        raise ValueError(f"no records in {path}")
    return records


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    position = (len(ordered) - 1) * fraction
    lower = int(position)
    upper = min(lower + 1, len(ordered) - 1)
    weight = position - lower
    return ordered[lower] * (1.0 - weight) + ordered[upper] * weight


def bootstrap_median_ci(values: list[float], resamples: int, seed: int) -> list[float]:
    generator = random.Random(seed)
    estimates = [
        statistics.median(generator.choices(values, k=len(values))) for _ in range(resamples)
    ]
    return [percentile(estimates, 0.025), percentile(estimates, 0.975)]


def process_summary(
    directory: Path,
    pattern: str,
    duration: Callable[[dict[str, Any]], float],
    expected_records: int,
    resamples: int,
    seed: int,
) -> dict[str, Any]:
    files = sorted(directory.glob(pattern))
    if len(files) != 5:
        raise ValueError(f"expected five process files for {pattern}, found {len(files)}")
    process_medians = []
    manifest = []
    schemas = set()
    for path in files:
        records = read_jsonl(path)
        if len(records) != expected_records:
            raise ValueError(f"expected {expected_records} records in {path}, found {len(records)}")
        durations = [duration(record) for record in records]
        process_medians.append(float(statistics.median(durations)))
        schemas.update(record["schema"] for record in records)
        manifest.append(
            {
                "file": path.name,
                "records": len(records),
                "sha256": sha256(path),
                "median_ns": process_medians[-1],
                "min_ns": min(durations),
                "max_ns": max(durations),
            }
        )
    median_ns = float(statistics.median(process_medians))
    return {
        "schemas": sorted(schemas),
        "process_medians_ns": process_medians,
        "median_of_process_medians_ns": median_ns,
        "process_median_range_ns": [min(process_medians), max(process_medians)],
        "bootstrap_95_ci_ns": bootstrap_median_ci(process_medians, resamples, seed),
        "raw_manifest": manifest,
    }


def paired_ablation(baseline: dict[str, Any], final: dict[str, Any], resamples: int, seed: int) -> dict[str, Any]:
    baseline_values = baseline["process_medians_ns"]
    final_values = final["process_medians_ns"]
    speedups = [base / candidate for base, candidate in zip(baseline_values, final_values)]
    speedup = float(statistics.median(speedups))
    return {
        "paired_process_speedups": speedups,
        "median_paired_speedup": speedup,
        "bootstrap_95_ci_speedup": bootstrap_median_ci(speedups, resamples, seed),
        "median_latency_reduction_percent": 100.0 * (1.0 - 1.0 / speedup),
    }


def main() -> None:
    args = arguments()
    if args.bootstrap_resamples <= 0:
        raise ValueError("--bootstrap-resamples must be positive")
    if not args.output.is_absolute() or args.output.exists():
        raise ValueError("--output must be an absent absolute path")

    fe2o3 = {
        key: process_summary(
            args.fe2o3_directory,
            f"{variant}-p*.jsonl",
            lambda record: float(record["timer"]["duration_ns"]),
            3_000,
            args.bootstrap_resamples,
            0x9500 + index,
        )
        for index, (key, variant) in enumerate(FE2O3_VARIANTS.items())
    }
    fla = {
        "decode": process_summary(
            args.fla_directory,
            "fla-t1-p*.jsonl",
            lambda record: float(record["timer"]["duration_ns_per_call"]),
            30,
            args.bootstrap_resamples,
            0x9510,
        ),
        "prefill": process_summary(
            args.fla_directory,
            "fla-t8-p*.jsonl",
            lambda record: float(record["timer"]["duration_ns_per_call"]),
            30,
            args.bootstrap_resamples,
            0x9511,
        ),
    }

    first_fe2o3 = read_jsonl(next(args.fe2o3_directory.glob("decode-explicit-key-reuse-p*.jsonl")))[0]
    first_fla = read_jsonl(next(args.fla_directory.glob("fla-t1-p*.jsonl")))[0]
    source = first_fe2o3["artifact"]
    fla_commit = first_fla["implementation"]["commit"]
    for summary in fe2o3.values():
        for item in summary["raw_manifest"]:
            records = read_jsonl(args.fe2o3_directory / item["file"])
            if any(record["artifact"]["source_commit"] != source["source_commit"] for record in records):
                raise ValueError("fe2o3 source commit changed within the campaign")
    for summary in fla.values():
        for item in summary["raw_manifest"]:
            records = read_jsonl(args.fla_directory / item["file"])
            if any(record["implementation"]["commit"] != fla_commit for record in records):
                raise ValueError("FLA commit changed within the campaign")

    decode_ablation = paired_ablation(
        fe2o3["decode_baseline"], fe2o3["decode_final"], args.bootstrap_resamples, 0x9520
    )
    prefill_ablation = paired_ablation(
        fe2o3["prefill_baseline"], fe2o3["prefill_final"], args.bootstrap_resamples, 0x9521
    )
    decode_final = fe2o3["decode_final"]["median_of_process_medians_ns"]
    prefill_final = fe2o3["prefill_final"]["median_of_process_medians_ns"]
    decode_fla = fla["decode"]["median_of_process_medians_ns"]
    prefill_fla = fla["prefill"]["median_of_process_medians_ns"]

    bandwidth_bytes_per_second = 8.0e12
    decode_bytes = 3_332
    prefill_bytes = 6_176
    decode_floor = decode_bytes / bandwidth_bytes_per_second * 1.0e9
    prefill_floor = prefill_bytes / bandwidth_bytes_per_second * 1.0e9
    result = {
        "schema": "fe2o3.gfx950.kda-mi350-performance.v1",
        "scope": {
            "device": "AMD Instinct MI350X gfx950:xnack- physical GPU 6",
            "shape": "FP32 B=1 H=1 K=16 V=16 with initial/final matrix state",
            "decode_tokens": 1,
            "prefill_tokens": 8,
            "cache_regime": "persistent allocations, repeated single-workgroup dispatch",
            "co_resident_allocation": "foreign idle vLLM allocation observed on physical GPU 6; no compute activity observed",
        },
        "provenance": {
            "fe2o3_source_commit": source["source_commit"],
            "fe2o3_source_tree": source["source_tree"],
            "fla_commit": fla_commit,
            "fla_implementation": "fused_recurrent_kda from official main",
        },
        "protocol": {
            "processes": 5,
            "initial_warmups": 1_000,
            "blocks": 30,
            "samples_per_block": 100,
            "fe2o3_timer": "ROCr HSA dispatch timestamps, one duration per dispatch",
            "fla_timer": "torch HIP event, 100-call block average",
            "statistic": "median of five independent process medians",
            "bootstrap_resamples": args.bootstrap_resamples,
            "timer_caveat": "both are device-side timers, but their sampling granularity differs",
        },
        "fe2o3": fe2o3,
        "fla": fla,
        "ablations": {
            "decode_explicit_key_reuse": decode_ablation,
            "prefill_c4_wy_ut": prefill_ablation,
            "negative_and_inapplicable": NEGATIVE_ABLATIONS,
        },
        "isa_evidence": ISA_EVIDENCE,
        "comparisons": {
            "decode_fe2o3_over_fla_speedup": decode_fla / decode_final,
            "prefill_fe2o3_over_fla_speedup": prefill_fla / prefill_final,
            "claim_boundary": "fastest measured eligible implementation for this exact shape and semantics; not a universal KDA SOTA claim",
        },
        "theoretical_bound": {
            "kind": "optimistic compulsory-HBM-only latency floor",
            "amd_mi350x_peak_hbm_bandwidth_bytes_per_second": bandwidth_bytes_per_second,
            "amd_source": "https://www.amd.com/en/products/accelerators/instinct/mi350/mi350x.html",
            "decode_unique_compulsory_bytes": decode_bytes,
            "decode_floor_ns": decode_floor,
            "decode_measured_over_floor": decode_final / decode_floor,
            "decode_bound_fraction_percent": 100.0 * decode_floor / decode_final,
            "prefill_unique_compulsory_bytes": prefill_bytes,
            "prefill_floor_ns": prefill_floor,
            "prefill_measured_over_floor": prefill_final / prefill_floor,
            "prefill_bound_fraction_percent": 100.0 * prefill_floor / prefill_final,
            "interpretation": "the floor excludes launch and instruction latency and assumes peak HBM bandwidth; one tiny head cannot saturate HBM, so dispatch and dependency latency dominate",
        },
    }
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(result["comparisons"], sort_keys=True))


if __name__ == "__main__":
    main()
