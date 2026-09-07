# MI300X R55 striped-wait CPU diagnostics, 2026-09-07

Status: `Exact-source diagnostic; no parity or causal claim`. This follow-up
measures calling-thread CPU consumption and context switches inside the KFD
striped-tail wait. It is deliberately separate from the accepted R54/R55
counterbalanced qualification set.

## Exact observation

- Commit: [`8be0ab47c6d96664169d488dbafdf38ee3dbcd05`](https://github.com/harsh-nod/fe2o3/commit/8be0ab47c6d96664169d488dbafdf38ee3dbcd05).
- Tree: `cefcf47075ea5244fea9b8afbf95daefcd8d04c8`.
- Device: GPU 2, PCI BDF `0000:46:00.0`, unique ID
  `0xd2e26fef80cf5c33`, KFD node 4, KFD GPU ID 29122.
- Workload: 1 MiB, depth 112, H2D then D2H, full-buffer validation,
  10 warmups and 30 retained samples per direction.
- Queue profiles: 14 logical queues in the combined directional profile and
  16 logical queues in the standalone striped profile. Both used two physical
  SDMA engines.
- Poll policy: 64 spin pauses, then 16 yields, then sleep requests capped at
  25 us while scanning every exact striped tail.

The diagnostic schema reports thread CPU and context-switch counters as
`Available`, `Unavailable`, or `Invalid`. All 30 samples for every measured
direction were `Available`.

## Results

All timing values below are separately reported p50 fields in nanoseconds.
They must not be subtracted as if they described one shared median sample.

| Queues | Direction | Submit | Wait | Tail wall | Tail thread CPU | Voluntary switches | Involuntary switches | Sleep requests | Requested sleep |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 14 | H2D | 97,385 | 2,643,047 | 2,622,536 | 227,019 | 32 | 0 | 32 | 800,000 |
| 14 | D2H | 93,159 | 2,529,578 | 2,510,980 | 214,942 | 31 | 0 | 31 | 775,000 |
| 16 | H2D | 101,822 | 2,615,827 | 2,594,405 | 238,408 | 31 | 0 | 31 | 775,000 |
| 16 | D2H | 98,487 | 2,514,105 | 2,496,949 | 227,862 | 31 | 0 | 31 | 775,000 |

Thread CPU is about 8.6%-9.2% of the corresponding tail-wall p50 field. Every
p50 involuntary-context-switch count is zero, while the voluntary-switch count
matches the sleep count in each row. These observations show that the caller
does not burn one CPU for the whole wait and that this run was sleep-heavy.

The tail-readiness diagnostics provide additional queue-shape context:

| Queues | Direction | First tail ready | All tails ready | Scan rounds | Tail observations |
| ---: | --- | ---: | ---: | ---: | ---: |
| 14 | H2D | 2,581,465 | 2,620,483 | 113 | 1,582 |
| 14 | D2H | 2,482,146 | 2,509,177 | 112 | 1,568 |
| 16 | H2D | 2,549,347 | 2,592,642 | 112 | 1,792 |
| 16 | D2H | 2,480,033 | 2,495,277 | 112 | 1,792 |

## Integrity

The retained nine-line log is
`/home/harsh/.codex-tmp/fe2o3-r55-cpu-diag-8be0ab47.log`, 26,313 bytes, with
SHA-256
`160e70ac9383a1b8a079770dd133a15531f619a0e9baa6f3685defe6e7d5f976`.
Independent canonical reconstruction validated its topology seal, both queue
monitor seals, and both exact row byte-count and SHA-256 bindings:

- topology seal:
  `fe3f37d829f89661660b9ab1e80d453237580a84f7676bad67bf0a70158e07e6`;
- q14 row:
  `47041ae1df916c61fedbc887e867891b0c241bda22817759c40b5ad0d5f35501`;
- q16 row:
  `f3f265041ae383e2b7611ef657c914dcb7dee2e5021caafaf0b8037cae91d288`.

Both monitors reported a clean target process-group census, no foreign or
terminal selected queues, observation gaps below the unchanged 10 ms limit,
successful target reaping, and process-group absence. The runner reported GPU
2 idle before each profile and idle after the run. Exact remote scratch and the
input bundle were removed after the log was retained locally.

## Interpretation boundary

The requested-sleep total is not avoidable latency. GPU execution progresses
while the caller sleeps, and this schema does not measure each wake-up's
overshoot beyond the instant the final tail became ready. Subtracting requested
sleep from wall time would therefore be invalid.

The prior R55 qualification showed that two logical queues met or beat HIP in
the measured 1 MiB cells while 4-16 logical queues did not. Together, the two
runs motivate testing native queue topology and bounded final-observation
policies separately; they do not prove either cause. This diagnostic did not
capture the full authenticated system/runtime identity or matched HIP/HSA
rows, so it cannot establish environment equivalence, performance parity,
application speedup, or an orders-of-magnitude result.
