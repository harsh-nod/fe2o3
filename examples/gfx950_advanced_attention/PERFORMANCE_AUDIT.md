# gfx950 attention performance audit

This audit covers the eight fixed-shape Rust kernels in `src/kernel.rs`. Every
number is a ROCr HSA dispatch timestamp from physical GPU 4 on `mi350`. The
campaign used 1,000 warmups followed by 30 blocks of 100 samples per artifact;
reported latency is the median of 3,000 dispatches and p95 is the sample p95.
Every timed artifact first passed the independent CPU oracle, immutable-input
checks, guard canaries, and post-block validation.

## Claim boundary

No row below establishes an external state-of-the-art result or a full-model
speedup. The public candidates do not expose the same shape, precision,
selection boundary, output ABI, cache regime, and timer. KDA has an exact
operator-level FLA comparator in the historical B=H=1 campaign, but the current
Rust launch evaluates four problems and therefore is not directly comparable
to that B=1 measurement. The other rows have no exact public comparator known
to this audit.

| Kernel | Public candidate to evaluate at production shape | Exact here? | Current median / p95 | Controlled ablation | Decision |
|---|---|---:|---:|---|---|
| KDA decode | FLA `fused_recurrent_kda` | No: current launch is B=4 | 6,920 / 7,120 ns | sequential 7,960 -> Wave16 6,920 ns, 1.157x | Keep Wave16 key reuse |
| KDA chunkwise prefill | FLA chunk/recurrent KDA | No: current launch is B=4, T=8, K=V=16 | 14,160 / 14,560 ns | sequential 17,880 -> C=4 WY/UT 14,160 ns, 1.263x | Keep two-chunk WY/UT |
| Content-selected sparse attention | FlashAttention/CK dense or block-sparse kernels | No: selector, top-3 and gate are fused | 30,080 / 30,520 ns | reciprocal 29,920 vs division 30,080 ns | Do not promote: direction reverses prior GPU6 result |
| Indexed DeepSeek sparse attention | public sparse-attention/FlashMLA implementations | No: teaching top-4 of 16, K=128, V=16 | 13,480 / 13,960 ns | lane exp 13,480 vs leader+broadcast 13,400 ns; reversed order ties at 13,080 ns | Keep lane-parallel exp; order-sensitive |
| Compressed hybrid attention | FlashAttention sliding-window or block-sparse kernels | No: three compressed blocks plus local window and gate | 28,000 / 29,323 ns | division 27,600 vs reciprocal 28,000 ns | Keep existing code: direction reverses prior GPU6 result |
| AttnRes aggregation | no exact standalone public kernel identified | No | 5,560 / 5,840 ns | explicit 5,600 vs bounded loop 5,560 ns; paired CI includes 1.0 | Keep bounded loop |
| Four-branch residual | compiler-generated fused elementwise kernels | No exact published artifact | 5,720 / 6,282 ns | explicit 5,640 vs loop 5,720 ns | Do not promote: prior GPU6 result tied |
| mHC Sinkhorn mix | no exact standalone public kernel identified | No | 6,880 / 7,120 ns | scalar matrix 9,760 -> distributed Wave16 6,880 ns, 1.419x | Keep distributed Sinkhorn |

## Optimization breakdown

KDA decode assigns each of the 256 threads one `(value, key)` state element,
reuses its key value for prediction and rank-one update, and uses uniform
Wave16 reductions. That reduces latency 13.57% from the exact sequential
baseline. KDA prefill keeps the state in registers and applies two ordered
four-token WY/UT transforms; exposing within-chunk reductions reduces latency
20.80%.

The two E4M3 attention kernels use one native m16n16k128 MFMA score tile per
Wave64. Each wave stages a private 2 KiB K tile in LDS and performs four
`ds_read_b64_tr_b8` transpose reads before the MFMA. A second LDS buffer is not
a pipeline at K=128 because there is no next K tile. Reciprocal rewrites are
retained as ablations, but their sub-2% direction changed between unlocked GPU4
and GPU6 campaigns, so neither is credited as a portable improvement.

DeepSeek indexed attention distributes each selected K=128 dot product over a
Wave16 and keeps the four reduction calls uniform across sentinel masks. The
leader-exp experiment reduced 64 OCML calls to four plus four lane exchanges,
but an order-reversed campaign erased the 0.61% forward-order gain. AttnRes and
the residual mixer are register-resident elementwise kernels; unrolling did not
produce a repeatable result. mHC distributes the 4x4 Sinkhorn matrix across a
Wave16, reuses one reciprocal per row, broadcasts column sums, and gains 29.51%
over the scalar-matrix baseline.

## Resource floor

The strict floor is `max(touched bytes / 8 TB/s, FP32 ops / 144.2 TFLOP/s,
FP8 ops / 4.6 PFLOP/s)`, using the provisional MI350X inputs already recorded
in `perf-evidence/mi350x-bound-inputs-v1.json`. It assumes whole-device peak
resources, perfect overlap, and no dispatch overhead. It is a resource lower
bound, not a prediction for these four-workgroup launches.

| Kernel | Touched bytes | Floor | Fastest measured / floor |
|---|---:|---:|---:|
| KDA decode | 13,328 | 1.666 ns | 4,154x |
| KDA prefill | 24,704 | 3.088 ns | 4,585x |
| Content sparse | 71,872 | 8.984 ns | 3,330x |
| DeepSeek sparse | 192,512 | 24.064 ns | 540x |
| Compressed hybrid | 71,680 | 8.960 ns | 3,080x |
| AttnRes | 36,864 | 4.608 ns | 1,207x |
| Four-branch residual | 40,960 | 5.120 ns | 1,102x |
| mHC Sinkhorn | 9,216 | 1.152 ns | 5,972x |

The gap is dominated by launch and small-problem underutilization, which this
roofline intentionally omits. A defensible efficiency claim needs a measured
empty-dispatch floor and production-sized shapes.

## Model applicability

These are architecture tutorials, not released-model replicas. KDA uses
K=V=16 rather than the production-width Kimi family configuration. The
DeepSeek slice uses 16 candidate tokens and top-k=4 rather than a production
sparse index domain. No verified public Kimi-K3, DeepSeek-v4, or GLM-5.3 model
configuration is encoded here. Consequently the audit reports no model-level
latency or throughput improvement. Production claims require new kernels at
the released model shapes and end-to-end measurements in the serving stack.

The complete structured evidence, artifact hashes, correctness tolerances, and
both DeepSeek experiment orders are in
`performance-audit-mi350-gpu4-v1.json`.
