#!/usr/bin/env python3
"""Benchmark the pinned FLA recurrent KDA comparator on one MI350X."""

from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
from pathlib import Path

import torch
import torch.nn.functional as F
import triton

from fla.ops.kda import fused_recurrent_kda


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tokens", type=int, choices=(1, 8), required=True)
    parser.add_argument("--warmups", type=int, default=1_000)
    parser.add_argument("--blocks", type=int, default=30)
    parser.add_argument("--samples-per-block", type=int, default=100)
    parser.add_argument("--block-rewarm", type=int, default=20)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--fla-repository", type=Path, required=True)
    parser.add_argument("--process", type=int, default=0)
    return parser.parse_args()


def scalar_reference(
    q: torch.Tensor,
    k: torch.Tensor,
    v: torch.Tensor,
    alpha: torch.Tensor,
    beta: torch.Tensor,
    initial_state: torch.Tensor,
) -> tuple[torch.Tensor, torch.Tensor]:
    state = initial_state[0, 0].double().cpu().clone()
    outputs = []
    for token in range(q.shape[1]):
        decay = alpha[0, token, 0].double().cpu().unsqueeze(1) * state
        error = v[0, token, 0].double().cpu() - k[0, token, 0].double().cpu() @ decay
        state = decay + beta[0, token, 0].double().cpu() * torch.outer(
            k[0, token, 0].double().cpu(), error
        )
        outputs.append(state.T @ (q[0, token, 0].double().cpu() * 0.25))
    return torch.stack(outputs).float(), state.float()


def main() -> None:
    args = arguments()
    if not args.output.is_absolute() or args.output.exists():
        raise ValueError("--output must be an absent absolute path")
    if min(args.warmups, args.blocks, args.samples_per_block) <= 0:
        raise ValueError("warmups, blocks, and samples must be positive")

    torch.manual_seed(0x950)
    device = torch.device("cuda", 0)
    shape = (1, args.tokens, 1, 16)
    q = F.normalize(torch.randn(shape, device=device, dtype=torch.float32), dim=-1)
    k = F.normalize(torch.randn(shape, device=device, dtype=torch.float32), dim=-1)
    v = torch.randn(shape, device=device, dtype=torch.float32) * 0.25
    alpha = 0.75 + 0.24 * torch.rand(shape, device=device, dtype=torch.float32)
    beta = 0.1 + 0.8 * torch.rand((1, args.tokens, 1), device=device, dtype=torch.float32)
    initial_state = torch.randn((1, 1, 16, 16), device=device, dtype=torch.float32) * 0.125
    g = alpha.log()

    def invoke() -> tuple[torch.Tensor, torch.Tensor]:
        return fused_recurrent_kda(
            q=q,
            k=k,
            v=v,
            g=g,
            beta=beta,
            scale=0.25,
            initial_state=initial_state,
            output_final_state=True,
            use_qk_l2norm_in_kernel=False,
            use_gate_in_kernel=False,
            use_beta_sigmoid_in_kernel=False,
            state_v_first=False,
        )

    output, final_state = invoke()
    reference_output, reference_state = scalar_reference(q, k, v, alpha, beta, initial_state)
    output_error = (output[0, :, 0].cpu() - reference_output).abs().max().item()
    state_error = (final_state[0, 0].cpu() - reference_state).abs().max().item()
    if not math.isfinite(output_error + state_error) or max(output_error, state_error) > 2.0e-5:
        raise RuntimeError(
            f"FLA comparator failed the independent reference: output={output_error} state={state_error}"
        )

    for _ in range(args.warmups):
        invoke()
    torch.cuda.synchronize()

    commit = subprocess.check_output(
        [
            "git",
            "-c",
            f"safe.directory={args.fla_repository}",
            "-C",
            str(args.fla_repository),
            "rev-parse",
            "HEAD",
        ],
        text=True,
    ).strip()
    records = []
    for block in range(args.blocks):
        for _ in range(args.block_rewarm):
            invoke()
        start = torch.cuda.Event(enable_timing=True)
        end = torch.cuda.Event(enable_timing=True)
        start.record()
        for _ in range(args.samples_per_block):
            invoke()
        end.record()
        end.synchronize()
        duration_ns = start.elapsed_time(end) * 1_000_000.0 / args.samples_per_block
        records.append(
            {
                "schema": "fe2o3.gfx950.kda-comparator-block.v1",
                "campaign_id": "kda-mi350-v2",
                "implementation": {
                    "id": "fla-fused-recurrent-kda",
                    "variant": "official-main",
                    "commit": commit,
                },
                "workload": {
                    "tokens": args.tokens,
                    "batch": 1,
                    "heads": 1,
                    "key_dimension": 16,
                    "value_dimension": 16,
                    "dtype": "fp32",
                    "state_layout": "logical-k-v",
                    "qk_l2_normalized_before_kernel": True,
                    "alpha_beta_activated_before_kernel": True,
                    "output_uses_updated_state": True,
                },
                "trial": {
                    "process": args.process,
                    "block": block,
                    "initial_warmups": args.warmups,
                    "block_rewarm": args.block_rewarm,
                    "calls_per_timed_block": args.samples_per_block,
                },
                "timer": {
                    "source": "torch-hip-event-block-average",
                    "duration_ns_per_call": duration_ns,
                },
                "correctness": {
                    "oracle": "independent-sequential-f64-matrix-state",
                    "output_max_absolute_error": output_error,
                    "state_max_absolute_error": state_error,
                    "tolerance": 2.0e-5,
                },
                "environment": {
                    "device": torch.cuda.get_device_name(0),
                    "torch": torch.__version__,
                    "rocm": torch.version.hip,
                    "triton": triton.__version__,
                    "rocr_visible_devices": os.environ.get("ROCR_VISIBLE_DEVICES"),
                },
            }
        )

    args.output.write_text(
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
        encoding="utf-8",
    )
    durations = sorted(record["timer"]["duration_ns_per_call"] for record in records)
    print(
        json.dumps(
            {
                "tokens": args.tokens,
                "blocks": len(durations),
                "median_ns": durations[len(durations) // 2],
                "min_ns": durations[0],
                "max_ns": durations[-1],
                "output_max_absolute_error": output_error,
                "state_max_absolute_error": state_error,
                "fla_commit": commit,
            },
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
