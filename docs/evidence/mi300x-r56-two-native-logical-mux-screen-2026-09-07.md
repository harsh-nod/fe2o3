# MI300X R56 two-native logical SDMA mux screen, 2026-09-07

Status: `Exact-source observational engineering screen`. This screen compares
one aggregate copy workload through the R56 two-native logical mux and the
existing combined striped profiles. The workload shape matches, but the queue
topology and ordering semantics do not. The observations are not a causal
speedup or parity result.

## Exact observation

- Commit: [`56adab9a2d0f4341b3afdc148c24282fa728d679`](https://github.com/harsh-nod/fe2o3/commit/56adab9a2d0f4341b3afdc148c24282fa728d679).
- Tree: `85d1d62a3ede116308eca169a0b76e673688a02e`.
- Binary SHA-256:
  `79c6925d4ce5e640e93b3c397d0cfdfc59e695b4f27e2c0db179d59aa21c2eed`.
- Host: `sharkmi300x-1`; driver `6.16.13`; pinned Rust and Cargo `1.97.1`.
- Device: GPU 2, PCI BDF `0000:46:00.0`, unique ID
  `0xd2e26fef80cf5c33`, KFD node 4, two ordinary SDMA engines.
- Workload: 1 MiB per request, depth 112, H2D then D2H, 10 warmups and
  30 retained samples in one process invocation per configuration.
- Timing: host `Instant` around submit and wait. Allocation and queue creation
  are outside the phase timing. Every round validates every byte.

The lane order alternated baseline/mux at lane 2, mux/baseline at lane 4,
baseline/mux at lane 8, and mux/baseline at lane 14. No lane was repeated in
both orders. Each reported p50 therefore describes repeated observations from
one queue/process setup, not independent experimental replicates.

## Observations

Positive deltas mean higher effective throughput for the logical-mux
configuration in this one screen.

| Logical lanes | H2D combined GB/s | H2D mux GB/s | H2D delta | D2H combined GB/s | D2H mux GB/s | D2H delta |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | 53.250 | 44.937 | -15.612% | 50.931 | 50.155 | -1.524% |
| 4 | 43.952 | 53.423 | +21.547% | 46.382 | 49.363 | +6.428% |
| 8 | 43.911 | 52.598 | +19.783% | 44.975 | 49.140 | +9.261% |
| 14 | 43.408 | 51.016 | +17.527% | 44.673 | 49.917 | +11.739% |

The lane-2 H2D difference was concentrated in the observed wait p50:
2,550,482 ns for the mux and 2,143,144 ns for combined striped, or +19.007%.
Submit p50 differed by +2.927%. This merits replication; it does not establish
a device-side cause.

For lanes 4, 8, and 14, mux H2D submit p50 differed by -4.156%, -25.709%,
and -41.294%, while wait p50 differed by -17.749%, -16.236%, and -13.745%.
D2H submit p50 differed by -10.881%, -27.857%, and -44.274%, while wait p50
differed by -5.931%, -7.966%, and -9.240%. These are host-wall observations
through different queue topologies, not isolated SDMA engine timings.

`striped16` was admitted only as a standalone 16-native-queue diagnostic. It
is not a `combined-striped16` baseline and is not compared by delta. Its H2D
and D2H p50 observations were 42.710 and 44.832 GB/s; `logical-mux16`
observed 53.075 and 49.614 GB/s.

## Integrity and limits

All 10 retained commands exited successfully, passed their applicable poll
smoke, validated the full buffers, and destroyed their queues. All 20 immediate
pre/post guards recorded the exact GPU identity, 0% sampled GPU use, 0% VRAM,
and no exact benchmark process. The final cleanup guard was also clean and the
exact remote scratch was removed.

The retained private archive is
`/home/harsh/.codex-tmp/fe2o3-r56-mi300x-screen-56adab9a-20260907T1834Z.tar.gz`,
934,367 bytes, with SHA-256
`3e7fdb0da8aa0e44a3816c302343be551af1df4246bb71ad0c371c5fb01bf58f`.
Its 55-row final manifest was independently rechecked. The archive contains
shared-host environment and process details and is not a public distribution
artifact without a separate privacy review.

The compared profiles have different native queue rosters and different
cross-lane ordering. The shared host also had substantial foreign CPU work;
guards sampled the selected GPU only before and after each invocation. There
are no authenticated device timestamps, continuous utilization records,
independent process replicates, or both-order repeats per lane. This screen
therefore establishes no causal speedup, statistical significance, device
bandwidth improvement, order independence, semantic equivalence, HIP/HSA
parity, general performance claim, or orders-of-magnitude result.
