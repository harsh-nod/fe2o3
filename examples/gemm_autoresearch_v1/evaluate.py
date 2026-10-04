#!/usr/bin/env python3
"""Fixed correctness and performance evaluator for the GEMM edit surface."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import statistics
import subprocess
import sys

EXAMPLE_DIR = Path(__file__).resolve().parent
REPO_ROOT = EXAMPLE_DIR.parents[1]
CANDIDATE = EXAMPLE_DIR / "src" / "kernel.rs"
RUNNER = EXAMPLE_DIR / "run-gfx942.sh"
PASS_RE = re.compile(
    r"^PASS gemm_autoresearch_v1: (?P<shape>[^,]+), .*max_abs_error=(?P<error>[0-9.eE+-]+)$",
    re.MULTILINE,
)
BENCH_RE = re.compile(
    r"^AUTORESEARCH_BENCH size=(?P<size>\d+) median_us=(?P<median>[0-9.]+) "
    r"p10_us=(?P<p10>[0-9.]+) p90_us=(?P<p90>[0-9.]+) gflops=(?P<gflops>[0-9.]+)$",
    re.MULTILINE,
)
SCORE_RE = re.compile(
    r"^AUTORESEARCH_SCORE geometric_mean_gflops=(?P<score>[0-9.]+)$",
    re.MULTILINE,
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def command_output(command: list[str]) -> str:
    return subprocess.check_output(command, cwd=REPO_ROOT, text=True).strip()


def parse_run(output: str) -> dict[str, object]:
    passed = PASS_RE.search(output)
    score = SCORE_RE.search(output)
    benches = [
        {
            "size": int(match.group("size")),
            "median_us": float(match.group("median")),
            "p10_us": float(match.group("p10")),
            "p90_us": float(match.group("p90")),
            "gflops": float(match.group("gflops")),
        }
        for match in BENCH_RE.finditer(output)
    ]
    if not passed or not score or len(benches) != 3:
        raise RuntimeError("runner did not emit the complete correctness and benchmark record")
    return {
        "correctness_shape": passed.group("shape"),
        "max_abs_error": float(passed.group("error")),
        "benchmarks": benches,
        "geometric_mean_gflops": float(score.group("score")),
    }


def append_ledger(path: Path, record: dict[str, object], note: str) -> None:
    header = "timestamp_utc\tcandidate_sha256\tlabel\tstatus\tmedian_geomean_gflops\tnote\n"
    if not path.exists():
        path.write_text(header, encoding="utf-8")
    fields = [
        str(record["timestamp_utc"]),
        str(record["candidate_sha256"]),
        str(record["label"]),
        str(record["status"]),
        f"{float(record['median_geometric_mean_gflops']):.2f}",
        note.replace("\t", " ").replace("\n", " "),
    ]
    with path.open("a", encoding="utf-8") as ledger:
        ledger.write("\t".join(fields) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--label", required=True)
    parser.add_argument("--note", default="")
    parser.add_argument("--replicates", type=int, default=3)
    parser.add_argument("--timeout-seconds", type=int, default=300)
    parser.add_argument("--output", type=Path, default=EXAMPLE_DIR / "results" / "latest.json")
    parser.add_argument("--ledger", type=Path, default=EXAMPLE_DIR / "results.tsv")
    args = parser.parse_args()
    if args.replicates < 1:
        parser.error("--replicates must be positive")

    visible = os.environ.get("ROCR_VISIBLE_DEVICES", "")
    if not visible or "," in visible:
        parser.error("set ROCR_VISIBLE_DEVICES to exactly one authorized GPU")

    target_dir = Path(
        os.environ.get(
            "FE2O3_AUTORESEARCH_TARGET_DIR",
            REPO_ROOT / "target" / "autoresearch-gemm-v1",
        )
    )
    artifact_dir = target_dir / "artifacts"
    temporary_dir = target_dir / "tmp"
    artifact_dir.mkdir(parents=True, exist_ok=True)
    temporary_dir.mkdir(parents=True, exist_ok=True)

    environment = os.environ.copy()
    environment.pop("HIP_VISIBLE_DEVICES", None)
    environment.update(
        {
            "FE2O3_BENCHMARK": "1",
            "FE2O3_ROOT_TARGET_DIR": str(target_dir),
            "FE2O3_OUTPUT_DIR": str(artifact_dir),
            "TMPDIR": str(temporary_dir),
        }
    )

    runs: list[dict[str, object]] = []
    raw_outputs: list[str] = []
    timestamp = dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat()
    for replicate in range(args.replicates):
        completed = subprocess.run(
            ["bash", str(RUNNER)],
            cwd=REPO_ROOT,
            env=environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=args.timeout_seconds,
            check=False,
        )
        raw_outputs.append(completed.stdout)
        if completed.returncode != 0:
            sys.stderr.write(completed.stdout)
            raise RuntimeError(f"replicate {replicate + 1} failed with status {completed.returncode}")
        run = parse_run(completed.stdout)
        runs.append(run)
        print(
            f"replicate={replicate + 1} correctness=pass "
            f"score_gflops={run['geometric_mean_gflops']:.2f}"
        )

    scores = [float(run["geometric_mean_gflops"]) for run in runs]
    record: dict[str, object] = {
        "schema": "fe2o3.gemm-autoresearch.mi300x.v1",
        "timestamp_utc": timestamp,
        "label": args.label,
        "status": "pass",
        "host": socket.gethostname(),
        "target": "gfx942:xnack-",
        "visible_device": visible,
        "base_commit": command_output(["git", "rev-parse", "HEAD"]),
        "candidate_sha256": sha256(CANDIDATE),
        "evaluator_sha256": sha256(Path(__file__)),
        "reference_sha256": sha256(EXAMPLE_DIR / "src" / "reference.rs"),
        "replicates": runs,
        "median_geometric_mean_gflops": statistics.median(scores),
        "measurement_boundary": (
            "Device-event timing on one visible MI300X. Re-run on an idle, clock-controlled "
            "host before making a performance or state-of-the-art claim."
        ),
        "raw_output_sha256": [
            hashlib.sha256(output.encode("utf-8")).hexdigest() for output in raw_outputs
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    append_ledger(args.ledger, record, args.note)
    print(f"median_geometric_mean_gflops={statistics.median(scores):.2f}")
    print(f"evidence={args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
