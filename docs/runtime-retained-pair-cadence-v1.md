# Retained XGMI Wait Cadence Experiment

This opt-in `hardware-diagnostic` experiment compares the existing 1 ms sleep
ceiling with a 25 microsecond ceiling. Ordinary runtime entrypoints keep their
existing policy. The 64-spin/16-yield prefix, single deadline, operational checks,
retained custody and final retirement behavior are unchanged.

`Gfx942XgmiRetainedWaitCadenceV1` selects `Ordinary1ms` or `Ceiling25us` for the
explicit retained-scope methods `wait_batch_for_cadence_experiment_v1` and
`wait_batch_for_cadence_diagnostic_v1`. The former disables timing observation;
the latter reports it. A requested sleep ceiling does not bound OS scheduling
latency. The separate producer and strict parser use a distinct experiment
schema; their results must not be relabeled as the ordinary benchmark schema.

## Qualification

Signed candidate `3527956b671b30b96ec34a20866584f0cee507bf` contains the seventeen
reviewed source paths. Its fresh CPU campaign passes all nineteen stages:
1,831 no-default and 1,837 all-feature KFD tests, no failures/ignores/filtering,
both non-test library checks, both strict all-target Clippy modes, source controls
and the example build. All original/retained executables and rejected attempts
are preserved. The CPU result SHA-256 is
`1c154c51a032c34d78b79ae6637772f9489a9c1bc242af854d3253e39b5269f7`.

The separately signed selector campaign passes a full relocated 2/0 proof,
three full 1/1 actual-body mutations with exact one/two/two diagnostic counts,
and both tool-release brackets. All six owned process groups close. The result
SHA-256 is `5b8a1feefe929a41e5b4825708c594349e8ae261c0c21f424706344cd6d3b9d9`.
The three-input proof includes a derived Clone obligation; it proves ceiling
selection, not cursor construction, scheduling, syscalls, hardware behavior or
performance. Macro diagnostic anchors establish source association, not branch
execution witnesses. Earlier diagnostic captures are not qualified negatives.

The raw CPU/proof packets are retained locally under
`/home/harsh/.codex-tmp/fe2o3-cadence-qualification-records-20261001-revision-3/`
and `/home/harsh/.codex-tmp/fe2o3-cadence-selector-qualification-20261001/`.
They are not yet a public, self-contained replay package. Integration preserves
the exact candidate paths and all four existing affected proof closures; only
the four wider source-inventory hashes are rebound.
All 24 local source-workflow commands pass on the integrated worktree, including
the two new cadence source/parser suites. This is not a new Rust or GPU run.

## MI300X Measurement

Signed source `1cfb7580f` completes a matched 36-invocation campaign on MI300X
GPUs 1 and 2: 24 ordinary and 12 instrumented invocations, 1 MiB copies, depths
1/16/32 and both directions. All 471 native, six transport, fourteen preparation
and four local preparation groups close. Independent readback checks the
original native archive, four fresh executables, complete raw receipts, both
durable archives and exact owned remote cleanup. The originals remain retained.

Each table entry is the mean of two ordinary invocation p50 batch latencies,
in microseconds. KFD uses the fifth ordered value of ten samples; HSA/HIP use
their validated producer-reported p50. These are not pooled medians.

| Depth | Direction | KFD 1 ms | KFD 25 us | HSA | HIP |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 | Forward | 40.2905 | 40.3005 | 33.5660 | 38.4020 |
| 1 | Reverse | 40.2750 | 39.8500 | 33.7150 | 38.3075 |
| 16 | Forward | 673.7175 | 471.9860 | 530.8135 | 494.9550 |
| 16 | Reverse | 673.1565 | 461.0495 | 530.5990 | 510.7895 |
| 32 | Forward | 1169.7285 | 916.4155 | 1065.0020 | 973.1395 |
| 32 | Reverse | 1167.8910 | 916.6255 | 1064.9925 | 988.6830 |

The 25 us experiment lowers depth-16 latency by 29.94-31.51% and depth-32
latency by 21.51-21.66% against the experiment's 1 ms control. In those deep
cells it has 11.08-13.95% lower latency than HSA and 4.64-9.74% lower than HIP.
Depth 1 remains 18.20-20.06% slower than HSA and 4.03-4.94% slower than HIP.
The control uses the separately named experiment method; the unchanged
ordinary `kfd-series` entrypoint was not timed in this campaign.

All 240 separately instrumented samples have complete diagnostics. Scan-thread
CPU medians rise from 73.82-74.40 to 76.98-77.41 us at depth 16, and from
117.59-117.60 to 137.38-137.64 us at depth 32. Median sleep counts rise from
four to five and from five to ten, respectively. Requested sleep totals are
counters, not actual sleep duration or quantified avoidable latency. These
instrumented CPU/phase distributions must not be pooled with ordinary timings.

The retained comparison is
`/mnt/c/fe2o3-native36-analysis-20261001/comparison.json`, SHA-256
`f02be1e2de6b775d80f3f97f3bf2a2671900587fcb6c28d71f10b784f06d2062`.
The independent root readback SHA-256 is
`1b6457a44d67bc87c41d07247815d0f4219b098c1518259c9aeb0451508693ee`.
The [compact public packet](evidence/dev-xgmi-retained-wait-cadence-native-2026-10-01/README.md)
includes the complete original native archive, comparison and independent
readbacks, signature records and signed source bundle. Original source/tool
prerequisites remain explicit; this is not self-contained controller replay.

## Remaining Work

The [shared numeric wait helpers](runtime-monotonic-wait-arithmetic-v1.md) now
have a separately qualified two-input proof at signed source `a7d110af1`:
four full 7/0 positives and seventeen intended 6/1 negatives. This covers the
actual numeric bodies, not the Duration/Instant adapter or complete cursor.
The earlier hardware measurement is not a benchmark of this arithmetic revision.

This is one point-admitted, nonexclusive shared-host campaign. Comparator
engine identities are unknown, and no device timeline or scheduler/cursor
refinement is established. Broader sizes, repeated campaigns, host CPU/tail
budgets, physical overlap and release thresholds remain open. The scoped
latency gain does not justify a default-policy change, general HIP/HSA parity,
orders-of-magnitude claims or an A7 milestone exit.
