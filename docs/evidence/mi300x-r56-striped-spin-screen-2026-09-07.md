# MI300X R56 striped-tail spin engineering screen, 2026-09-07

Status: `Exact-source engineering screen; spin hypothesis rejected`. This
screen tests whether replacing the profiled striped-tail wait's bounded sleeps
with longer active-spin floors materially closes the observed 1 MiB HIP gap.
It does not.

## Exact observation

- Commit: [`71ab99c3740f4fb9170fec1e7cd8c9b9cbf2eba5`](https://github.com/harsh-nod/fe2o3/commit/71ab99c3740f4fb9170fec1e7cd8c9b9cbf2eba5).
- Tree: `8937051aee0111b70716c615921f1eb10b341949`.
- Binary SHA-256:
  `c2c0cc639ec3fe6997c171788f76ce7ad34d0a78c2c8fe875ead7042534ea14f`.
- Host: `sharkmi300x-1`, Linux `6.8.0-124-generic`, ROCm `7.2.4`.
- Device: GPU 2, `gfx942:xnack-`, PCI BDF `0000:46:00.0`, unique ID
  `0xd2e26fef80cf5c33`, KFD node 4, KFD GPU ID 29122.
- Workload: 1 MiB per copy, depth 112, H2D then D2H, full-buffer validation,
  10 warmups and 30 retained samples per direction.
- Queue profiles: 14 logical queues in the combined profile and 16 logical
  queues in the standalone profile; both use two physical SDMA engines.
- Placement: measurement CPUs 0-47 and NUMA node 0; queue observer CPU 95.

The q14 screen covers `current`, `250us`, `500us`, `1ms`, `1500us`, and
`3ms`. The q16 confirmation compares `current` with `3ms`. Every phase began
and ended with GPU 2 at zero reported utilization. The retained host guard
records report `status=clean`, no foreign or terminal selected-GPU queues,
successful target exit and reaping, process-group absence, and maximum census
gaps of 5,127-6,186 us under the 10,000 us bound.

This is one sequential diagnostic screen, not a counterbalanced HIP/HSA
qualification. Its profiled timestamp, thread-clock, and rusage observations
also perturb the measured path.

## Results

All values are independently reported p50 fields in nanoseconds except the
sleep count. A negative delta is lower E2E than that profile's `current` row.

| Queues | Budget | H2D E2E | H2D delta | H2D thread CPU | H2D sleeps | D2H E2E | D2H delta | D2H thread CPU | D2H sleeps |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 14 | current | 2,685,299 | baseline | 218,508 | 31 | 2,628,204 | baseline | 210,925 | 31 |
| 14 | 250us | 2,706,151 | +0.777% | 387,941 | 29 | 2,606,421 | -0.829% | 379,552 | 29 |
| 14 | 500us | 2,694,422 | +0.340% | 620,853 | 26 | 2,606,352 | -0.831% | 616,675 | 25 |
| 14 | 1ms | 2,714,102 | +1.073% | 1,090,296 | 20 | 2,625,130 | -0.117% | 1,085,366 | 19 |
| 14 | 1500us | 2,674,353 | -0.408% | 1,563,187 | 13 | 2,644,689 | +0.627% | 1,559,829 | 13 |
| 14 | 3ms | 2,686,932 | +0.061% | 2,559,151 | 0 | 2,577,969 | -1.911% | 2,459,331 | 0 |
| 16 | current | 2,723,116 | baseline | 234,940 | 31 | 2,669,095 | baseline | 226,597 | 31 |
| 16 | 3ms | 2,685,469 | -1.382% | 2,551,109 | 0 | 2,613,593 | -2.079% | 2,491,599 | 0 |

At q14 the H2D result has no material or monotonic improvement, while the best
D2H row is 1.911% lower. At q16 the 3 ms row is 1.382%-2.079% lower. In
exchange, the 3 ms floor raises tail-scan thread CPU by 10.86x-11.71x and
eliminates voluntary sleep. This is not a competitive trade and does not
explain the larger HIP gap.

The next performance experiment therefore targets the logical-to-physical
queue architecture: two persistent native queues, two publications, two
doorbells, and two tails, while preserving logical lane assignment and exact
request custody. This screen does not establish that the mux will improve
performance.

## Integrity and limits

The retained archive is
`/home/harsh/.codex-tmp/fe2o3-r56-spin-engineering-screen-71ab99c3.tar.gz`,
28,846 bytes, with SHA-256
`9a8b928df7317cd66c0270534cb18a8adbad895d50d652240e13454dbd51daab`.
Its 25-entry internal SHA-256 manifest was independently rechecked after
extraction. The context record has SHA-256
`2d1bb7233ed53afd56bb8a027a5efe74789069dc5b7f08e30238ef6eb090a0a3`.
The exact remote source, target, result, and monitor scratch directory was
removed after local retention.

GPU work progresses while the host sleeps. Requested sleep time is not actual
sleep time, and neither wall time nor thread CPU isolates scheduler wake-up,
native queue scheduling, or device execution. This screen adds no HIP/HSA
parity, speedup, causal, fairness, liveness, or device-timestamp claim.
