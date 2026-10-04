# GEMM autoresearch loop

This example adapts the single-edit-surface, fixed-evaluator pattern from
Karpathy's autoresearch project to an ordinary safe Rust GPU kernel. The agent
edits only `src/kernel.rs`; `evaluate.py` owns compilation, correctness, timing,
and the experiment ledger.

The candidate computes dynamic row-major BF16/BF16/F32 GEMM:

```text
C = alpha * A * B + beta * C
```

One wave64 workgroup owns a 16x16 output tile. The current candidate loads MFMA
fragments directly because the retained MI300X experiment found that a two-slot
LDS pipeline added cost without cross-wave reuse for this mapping. The rejected
pipeline is retained in `experiments/double_buffer.rs`.

## Run on MI300X

Choose one GPU that you are authorized to use and expose only that device:

```bash
ROCR_VISIBLE_DEVICES=5 python3 evaluate.py --label direct-mfma
```

The evaluator runs three fresh production passes. Each pass requires:

```text
safe Rust -> semantic MIR -> ranked PLIRON -> Kernel IR
          -> gfx942 LLVM -> HSACO -> launch -> CPU oracle
```

Only a passing candidate is timed. Eleven device-event samples are collected at
each of 256x256x256, 512x512x512, and 1024x1024x1024. Repeated launches amortize
the event resolution, and the scalar score is the geometric mean of throughput
across the three sizes. The final score is the median across processes.

Copy `results.example.tsv` to the untracked `results.tsv` only if you want a
fresh ledger template. Build products live under `target/autoresearch-gemm-v1`.
Remove that directory when the session ends on a shared machine.

## Evidence boundary

The checked-in MI300X record demonstrates that the tutorial and two displayed
variants compiled, lowered, launched, and passed the bounded oracle. The timing
was collected while unrelated workloads occupied every GPU, so it teaches the
decision protocol but is not a clean performance publication or a SOTA claim.
