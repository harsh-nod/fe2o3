# MI300X R54/R55 striped SDMA qualification, 2026-09-07

Status: `Exact-product workload qualification; bounded HIP parity not
demonstrated`. R54 caps each striped-tail wait sleep request at 25 us. R55
adds fail-closed whole-phase relaunch for an invalid KFD census. The complete
counterbalanced run passed functional and evidence validation. KFD beat HSA in
every measured cell, but the preregistered HIP parity and 10x gates failed.

## Exact product and method

- Exact hardware-run commit: `a193a43f34988c1e0b652f496e3d464a520ba43a`,
  tree `ffd78709d16eee873784b05a6878aa023c97c80b`.
- Published source-equivalent R55 commit:
  [`d6a4134016513ef4ab230dfe86fd57930c7351d3`](https://github.com/harsh-nod/fe2o3/commit/d6a4134016513ef4ab230dfe86fd57930c7351d3),
  with the same tree and stable patch ID
  `7cc88a41bd54c6a8eb88d4e42936ed07d1577a09`.
- R54 runtime commit:
  [`07c467da9988de65c9202db2dda3e40d892aabfb`](https://github.com/harsh-nod/fe2o3/commit/07c467da9988de65c9202db2dda3e40d892aabfb).
- Sealed environment: Linux `6.8.0-124-generic`, ROCm `7.2.4`.
- Device: GPU 2, `gfx942:xnack-`, unique ID `0xd2e26fef80cf5c33`.
- Workloads: 4 KiB and 1 MiB copies, depth 112, logical queue counts 2, 4,
  8, 14, and 16, in both H2D and D2H directions.
- Each phase used 10 warmups and 30 retained samples. Three slots rotated both
  backend and workload order, for 90 admitted phases and 2,700 retained
  backend/workload rounds. Each round retains both H2D and D2H measurements.
- KFD used exactly two physical SDMA engines. HSA physical-engine placement
  was not observed; HIP used nonblocking streams. Allocation and queue creation
  were outside the timed region for all backends.

The bounded-parity criterion requires, for every matched cell, median KFD
latency at most 1.10x the reference, median KFD bandwidth at least 0.90x the
reference, and every slot's latency ratio at most 1.20. The 10x criterion
requires every KFD latency ratio to be at most 0.10.

## Results

The table reports the median paired-slot E2E latency ratio. Lower is better;
values below 1.0 favor KFD.

| Bytes | Queues | Direction | KFD / HSA | KFD / HIP |
| ---: | ---: | --- | ---: | ---: |
| 4096 | 2 | H2D | 0.291999 | 1.171618 |
| 4096 | 2 | D2H | 0.247986 | 0.925777 |
| 4096 | 4 | H2D | 0.297541 | 1.034535 |
| 4096 | 4 | D2H | 0.255121 | 0.931100 |
| 4096 | 8 | H2D | 0.318246 | 1.184897 |
| 4096 | 8 | D2H | 0.274555 | 0.985234 |
| 4096 | 14 | H2D | 0.339961 | 1.088741 |
| 4096 | 14 | D2H | 0.297296 | 0.906922 |
| 4096 | 16 | H2D | 0.290687 | 0.886683 |
| 4096 | 16 | D2H | 0.305807 | 0.885024 |
| 1048576 | 2 | H2D | 0.609200 | 0.829085 |
| 1048576 | 2 | D2H | 0.688779 | 0.973682 |
| 1048576 | 4 | H2D | 0.721486 | 1.142309 |
| 1048576 | 4 | D2H | 0.751168 | 1.159610 |
| 1048576 | 8 | H2D | 0.732118 | 1.222167 |
| 1048576 | 8 | D2H | 0.755353 | 1.230190 |
| 1048576 | 14 | H2D | 0.756962 | 1.291257 |
| 1048576 | 14 | D2H | 0.767870 | 1.247764 |
| 1048576 | 16 | H2D | 0.751315 | 1.273139 |
| 1048576 | 16 | D2H | 0.766209 | 1.250071 |

KFD was 2.94x-4.03x faster than HSA for 4 KiB and 1.30x-1.64x faster
for 1 MiB in this scope. Against HIP, the result ranges from 0.829x to 1.291x
latency. Several cells favor KFD, but the full matrix does not pass the bounded
gate.

Compared with the prior R40 evidence, the 1 MiB q16 KFD/HIP latency ratio fell
from 1.492265 to 1.273139 for H2D and from 1.493142 to 1.250071 for D2H,
reductions of about 14.7% and 16.3%. The corresponding q14 ratios fell from
1.494843 to 1.291257 and from 1.487609 to 1.247764. These before/after results
are consistent with a useful wait-policy improvement, but they do not isolate
the sleep cap as causal. The operating system can wake later than a 25 us
request, and clocks, scheduling, and other unobserved conditions can differ
between runs.

## Retry qualification

Four attempts exceeded the unchanged 10 ms census-observation bound. Each was
discarded before publication, with reason `observation-gap-exceeded`; the guard
proved target process-group absence and deleted target output before returning
the typed retry. The runner observed GPU 2 idle and relaunched the whole phase.
All four records, their per-phase counts, and transcript digests are retained
in the accepted slot logs. No phase continued through a gap, and no missed
census was accepted as clean evidence.

## Integrity

The external archive is retained at
`/home/harsh/.codex-tmp/fe2o3-r55-r40-a193a43f-ffd78709.tar.gz`. It is
12,224,586 bytes with SHA-256
`17b80d44ff1a6dd8e6cb263c89dc44207db4e4db3eab7a553e8bc5f99dd2d03a`.
After the archive streamed locally, an operator cleanup check observed the
exact shared-host checkout, input bundle, runner scratch, and output paths
absent and all eight GPUs idle. That post-stream observation is not part of the
sealed archive.

- Counterbalance set ID:
  `63c8bd971dce85d70b5f92e52d33a7a6a9fb1bf56636f41b25170194d7dbe483`.
- Manifest SHA-256:
  `8a1f4cd0a2ae59a219ae52c9211d4034c377b25e64f36cb4d278858c7a49b0c4`.
- Set-validation SHA-256:
  `836c9de6ad43ca845cd3995e33cc03f358149851d65a1882de634fde7a5a9b5a`.
- Source archive SHA-256:
  `b21b566f93db0eaeeb20eca3bcdaba45b8361832e17c6911956b9c54f785d018`;
  it exactly matches `git archive` of the hardware-run commit.
- Slot SHA-256 values: slot 0
  `6e9c3626ada0f63124202c9bb77bbecd1c1ba5c95b54caac6c4ad2880a7c0120`,
  slot 1
  `8614a49d7316de825b1afdef1b5eb086d0ae4238d8c3979caa021195a2c838ba`,
  and slot 2
  `2784b2b5c22b3281dacef34fa89d32d781dcdc51bcb7c5ed86d08af2361dd491`.

The archive manifest, exact checker rerun, source match, functional checks, and
all 90 phase records passed independently after extraction.

## Next measurement

The remaining large-copy deficit is still dominated by KFD's host completion
wait rather than submission. The next exact diagnostic records calling-thread
CPU time and voluntary/involuntary context switches around the tail loop, in
addition to wall time, scan rounds, wait actions, requested sleeps, first/all
tail readiness, audit, and retirement. It is diagnostic-only and must run
outside this accepted counterbalance set.

## Claim limits

This qualifies one source tree, host, GPU, driver stack, API shape, and
striped-copy workload. E2E is a host-monotonic interval; it is not a physical
link or pure device interval. Boundary load checks cannot prove in-phase
exclusivity. The result does not establish causality, generic copy performance,
runtime-wide HIP/HSA parity, or an orders-of-magnitude speedup.
