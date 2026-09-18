# MI300X HSA Pool And Engine Diagnostic

This development experiment measures the standalone HSA pool/engine diagnostic
and unchanged KFD runtime-facade anchors at source
`3da2d25ac965afa9845b8ab1246e5fdf13c5d821`. It adds no production runtime change,
broader native/formal milestone acceptance, HIP comparison, or HIP/HSA parity
claim. Native R125 CPU/test, Admission R118B C1-C3 and Resources R116/V3 remain
the accepted checkpoints; A1/A2 and #182 remain open.

## Results

The complete campaign exited zero: 24 measurement processes, 49 campaign
occupancy observations, all 208 recorded HSA warmup/sample rounds, and all eight
KFD anchors passed. Ten analysis tests, including rejection mutations and exact
arithmetic fixtures, passed. The source/receipt auditor and Python lint passed.
All sixteen recorded commands exited zero; no failed workload was omitted.
Two independent read-only reviews passed. They checked the source/receipt
closure and independently recomputed the HSA percentiles and paired ratios;
they did not rerun GPU workloads or the archived qualification helpers.

Each entry below is the median of process-level p50 latencies, **not** a pooled
sample percentile. HSA has four processes per cell; KFD has eight anchors.

| Configuration | H2D ms | D2H ms |
| --- | ---: | ---: |
| HSA A: fine, mask 1 | 4.880 | 5.609 |
| HSA B: fine, mask 2 | 4.819 | 4.858 |
| HSA C: coarse, mask 1 | 4.838 | 5.604 |
| HSA D: coarse, mask 2 | 4.853 | 4.856 |
| KFD unchanged facade | 6.063 | 5.070 |

For each block, the mean of the before/after KFD p50 anchors is divided by that
block's HSA p50. Medians of these four paired ratios are:

| KFD / HSA latency | A | B | C | D |
| --- | ---: | ---: | ---: | ---: |
| H2D | 1.242x | 1.259x | 1.253x | 1.249x |
| D2H | 0.904x | 1.044x | 0.905x | 1.044x |

Within HSA, mask 2 / mask 1 D2H latency is 0.865x for fine and 0.867x for
coarse pools: about 13.3-13.5% lower latency in this setting. The corresponding
H2D ratios are 0.988x and 1.003x. Coarse/fine paired median ratios remain within
0.8% of one for either direction and requested mask. These observed pool-grain
differences do not remove the 24-26% KFD H2D gap; they do not establish cache
equivalence or rule out other memory-mapping effects.

KFD after/before anchor ratios are 0.9957-1.0023 for H2D and 0.9981-1.0031 for D2H
across the four blocks. These endpoint observations do not establish continuous
host isolation or statistical confidence intervals. Selecting only the slower
HSA mask-1 D2H result would give a misleading broad parity claim. The CPU-agent
and host-timing qualifications below are essential to interpreting both tables.

## Workload And Admission

- Physical GPU 4, UID `0x54f88318ca05093d`, PCI `0000:85:00.0`, `gfx942`.
- ROCm `/opt/rocm-7.2.4`; disabled XNACK for the HSA process.
- 268,435,456 bytes, depth one, three warmups and ten measured rounds per process.
- Fixed external CPU affinity 48-95 and memory policy node 1, the GPU's reported
  local CPU list/NUMA node. The HSA CPU-agent index remains explicitly zero.
- HSA A = fine/mask 1, B = fine/mask 2, C = coarse/mask 1, D = coarse/mask 2.
  Four block orders are `A B D C`, `B C A D`, `C D B A`, `D A C B`.
  An unchanged KFD facade process precedes and follows each block.
- Every process has immediate preflight and postflight checks for exact device
  UID/BDF, utilization zero, less than 512 MiB used VRAM and no mapped PID.
  Twenty seconds separate process exit and postflight, retaining the previously
  qualified deferred-reclamation cooldown without changing guard thresholds.

The user permitted available GPUs on this shared host. These are point-in-time
occupancy observations, not exclusive reservations or continuous isolation.
Other GPUs and host activity were not stopped. A GPU with low utilization but
retained foreign allocations was not treated as free.

HSA CPU agent 0 reports driver node 0 and is **not** the reported nearest CPU
agent for GPU driver node 6. The external NUMA policy therefore must not be
called proof of physical HSA-pool locality. Agent/pool handles are process-local
observations, not cross-process physical identities.

## Measurement Boundaries

HSA records every warmup and sample's host submit, wait-plus-reset and total
interval, and verifies every returned byte against a changing uniform pattern.
Only exact-zero completion permits signal reset. The final completion record
follows all signal destruction, frees and HSA shutdown. The extra intermediate
clock observation is diagnostic overhead. Percentiles are nearest-rank within
each process; warmups are excluded, and process samples are never pooled.

KFD uses the public RuntimeContext with its optional version journal disabled,
65 packets in two bounded windows, H2D queue index 1, D2H index 0, and the
unchanged 50-microsecond progress-wait slice. It reports host elapsed copy
intervals and validates the complete returned buffer. Initialization, shadow
preparation and validation are outside the copy timer. Its output has aggregate
percentiles, not the HSA diagnostic's per-round submission breakdown.

The experiment can compare requested HSA pool policy and engine mask within the
fixed CPU-agent/placement setting. It cannot establish that HSA masks 1/2 are
the same physical engines as KFD indices 0/1, that HSA pools have the same cache
attributes or physical residency as KFD coherent GTT, or that the remaining
facade latency is pure wrapper overhead. No device timeline is recorded.

## Next Controls

First repeat with an explicitly enumerated HSA CPU index whose reported handle
equals `nearest_cpu_agent`, holding the other factors fixed. That tests pool
owner selection, not physical page residency. Then add a separately named KFD
diagnostic that splits `copy_async` submission from progress and counts wait/
flush calls, comparing the current 50-microsecond slice with a remaining-deadline
wait. Preserve the same Context, owners, currentness checks, two-window
continuation, validation and teardown. This would measure re-entry/wait-policy
effects, not pure wrapper overhead or device time.

The full KFD host-image copy/hash/seal occurs in `write_allocation` before the
timed call, and D2H shadow readback is after both copy timers. The unchanged
single-packet lower benchmark cannot supply a 256 MiB control: its maximum is
4,194,272 bytes. These follow-ups are not implemented or measured by this record.

## Evidence And Reproduction

`prepare.sh` builds the pinned runtime example and real ROCr-linked diagnostic
in one owned temporary directory. No CPU mock is linked into that executable.
The source manifest covers the selected Cargo/toolchain, crate, example and
benchmark paths, not a complete compiler or dynamic-loader closure. Compiler,
kernel and selected ROCm library/header identities are retained separately.

`raw/` preserves command arguments, start/finish timestamps, outputs and exit
codes. Standard-input bodies used for `create` and `occupancy-after-cleanup` are
archived as `create.sh` and `guard.sh`, respectively; the recorded command vectors
alone do not authenticate those stdin bytes. The initial `stage` and `prepare`
transport commands briefly overlap; staging completed before compilation.
Cleanup was tightened to the exact created canonical path and restaged before
the GPU campaign. The executed guard and runner were not changed during it.

The read-only analysis commands are:

```sh
python3 -I docs/evidence/dev-hsa-pool-engine-mi300x-2026-09-18/test-summary.py -v
python3 -I docs/evidence/dev-hsa-pool-engine-mi300x-2026-09-18/summarize.py
python3 -I docs/evidence/dev-hsa-pool-engine-mi300x-2026-09-18/audit.py
```

The audit authenticates selected source bytes against the exact built Git
commit, before/after source-check logs, harness identities, executable digest
observations, command vectors, receipt ordering, raw occupancy observations,
complete process/round rosters, selected pool eligibility, summary reproduction
and the exact owned-directory cleanup record. It does not prove native execution
from source or make unsigned console observations into hardware attestation.

The source audit matched all 5,524 selected files to the exact built commit.
The KFD executable SHA-256 is
`2434713c961ec91304ccf22f8d1cf61d7af19c998a871548baf05a6ec5b94727`;
the HSA diagnostic executable SHA-256 is
`5d7e0578ea8078bf10066bbd7f36fe78608449af6fb6d647f2ec4503e86d9e33`.
Independent post-run hash observations match the build manifest and successful
before/after checks. Binaries are not retained in this source/evidence archive.

Only the owned directory
`/tmp/fe2o3-hsa-pool-engine-20260918.wBkvxkrs` was removed, recovering the reported
approximately 407 MiB. Cleanup checked the exact canonical path, owner marker and absence of
live executables/cwds rooted there before deletion. The follow-up device
occupancy observation passed. No foreign process was signaled or file removed.
