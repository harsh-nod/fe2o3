# MI300X R40 striped SDMA scaling, 2026-09-07

Status: `Exact-product workload qualification; bounded parity not demonstrated`.
R40 tests native KFD striped copies against matched HSA asynchronous copies and
HIP nonblocking streams over two transfer sizes and five logical queue counts.
KFD beat HSA in every measured cell, but the preregistered bounded-parity gate
failed against HIP. The preregistered 10x gate also failed.

## Exact product and method

- Exact production commit:
  [`a4c2da0fa3072c554ac2d62426f4ea4f68fb58db`](https://github.com/harsh-nod/fe2o3/commit/a4c2da0fa3072c554ac2d62426f4ea4f68fb58db),
  tree `c475a9204a6e6201acde3dc502f256b1a49cdd81`.
- Host: `sharkmi300x-1`, Linux `6.8.0-124-generic`, ROCm `7.2.4`.
- Device: GPU 2, `gfx942:xnack-`, unique ID `0xd2e26fef80cf5c33`.
- Workloads: 4 KiB and 1 MiB copies, depth 112, logical queue counts 2, 4,
  8, 14, and 16, in both H2D and D2H directions.
- Each backend/workload phase used 10 warmups and 30 retained samples. Three
  slots used cyclic backend order and forward/reverse/rotated workload order,
  for 90 phases total.
- KFD used exactly two physical SDMA engines. Additional logical queues were
  striped over those engines. HSA physical-engine placement was not observed;
  HIP used nonblocking streams.
- Allocation and queue creation were outside the timed region. Submission was
  serial on the host for all three backends. Full buffers were validated after
  every round.

The bounded-parity criterion required, for every matched cell, median KFD
latency at most 1.10x the reference, median KFD bandwidth at least 0.90x the
reference, and every slot's KFD latency at most 1.20x the reference. The 10x
criterion required KFD latency at most 0.10x every matched reference cell.

## Results

The table reports the median paired-slot E2E latency ratio. Lower is better;
values below 1.0 favor KFD.

| Bytes | Queues | Direction | KFD / HSA | KFD / HIP |
| ---: | ---: | --- | ---: | ---: |
| 4096 | 2 | H2D | 0.302067 | 1.156665 |
| 4096 | 2 | D2H | 0.317906 | 1.124048 |
| 4096 | 4 | H2D | 0.308137 | 1.065391 |
| 4096 | 4 | D2H | 0.324479 | 1.048156 |
| 4096 | 8 | H2D | 0.326337 | 1.230674 |
| 4096 | 8 | D2H | 0.343595 | 1.240012 |
| 4096 | 14 | H2D | 0.344660 | 1.097057 |
| 4096 | 14 | D2H | 0.359981 | 1.078870 |
| 4096 | 16 | H2D | 0.353637 | 1.051756 |
| 4096 | 16 | D2H | 0.373604 | 1.051293 |
| 1048576 | 2 | H2D | 0.846179 | 1.140486 |
| 1048576 | 2 | D2H | 0.885850 | 1.314767 |
| 1048576 | 4 | H2D | 0.850686 | 1.351330 |
| 1048576 | 4 | D2H | 0.886941 | 1.374722 |
| 1048576 | 8 | H2D | 0.862080 | 1.459777 |
| 1048576 | 8 | D2H | 0.896331 | 1.468758 |
| 1048576 | 14 | H2D | 0.873557 | 1.494843 |
| 1048576 | 14 | D2H | 0.912677 | 1.487609 |
| 1048576 | 16 | H2D | 0.877374 | 1.492265 |
| 1048576 | 16 | D2H | 0.916836 | 1.493142 |

For 4 KiB, KFD was 2.68x-3.31x faster than HSA. For 1 MiB, it was
1.09x-1.18x faster than HSA. Against HIP, some 4 KiB cells satisfied the
bounded gate, but the overall matrix did not. No 1 MiB HIP cell satisfied the
gate.

At 1 MiB, the aggregate payload divided by host E2E median changed as follows
from queue count 2 to 16. These are workload rates, not physical-link
bandwidth measurements.

| Backend | H2D q2 | H2D q16 | D2H q2 | D2H q16 |
| --- | ---: | ---: | ---: | ---: |
| KFD | 38.260 GB/s | 37.300 GB/s | 38.420 GB/s | 37.437 GB/s |
| HIP | 43.635 GB/s | 55.676 GB/s | 50.495 GB/s | 55.892 GB/s |

KFD E2E latency worsened by approximately 2.6% from q2 to q16. HIP H2D
latency improved by approximately 21.6% and HIP D2H latency by approximately
9.7%. In the median q16 slot, KFD submission was about 153 us shorter than HIP
in H2D and 155 us shorter in D2H, while KFD completion wait was about 1.19 ms
longer in both directions. This localizes the observed large-copy deficit to
post-submission scheduling/completion behavior rather than host submission,
but does not identify its cause.

## Integrity

The external, non-durable archive is retained below
`/home/harsh/.codex-tmp/fe2o3-r40-evidence-a4c2/`, with basename
`r40-striped-2d8f090734e156948a1f4446335014932918f317939d2ede97ead797799d9711.tar.gz`.
It is 12,177,021 bytes with SHA-256
`c73b3c136d474b46402048d1daedb8caa72a0f1157bebbfb7e5e3501d9ff1132`.
The shared MI300X checkout, runner scratch, and output were removed after the
archive was copied and verified.

- Counterbalance set ID:
  `2d8f090734e156948a1f4446335014932918f317939d2ede97ead797799d9711`.
- Manifest SHA-256:
  `0750ec2822c356835f280205baf20d986cee8411a810fef58f7598e11a2bcc2b`.
- Set-validation SHA-256:
  `5df5e98936954e9e09efe2f783578f75382ee505176f5c19a8cb5536264bf789`.
- Source archive SHA-256:
  `0318306ff00be2631337a4bfa390047f34cefa576a5f0e242ab7aac42bab93c5`;
  it is an exact `git archive HEAD` of the product commit.
- Slot SHA-256 values: slot 0
  `5e530447143e2e3b9c9ca1c7230c7ef338c21d32ee245aeddc7f5f9821ff9239`,
  slot 1
  `09ae62fd7a9b77bbc2de45bd86fbde2149ded900875aefa5ffe73759eca0d3d1`,
  and slot 2
  `e5050deac5ee69e9e9b11795308c863b730613fe8beb2fc701992220408f9397`.

All functional checks, slot checks, set validation, archive hashing, internal
hashing, source matching, and gzip validation passed. All 180 boundary
snapshots observed GPU 2 idle, with FCLK 1300 MHz, MCLK 900 MHz, and boundary
power of 146-148 W. Boundary telemetry does not establish in-phase clocks,
power, or occupancy.

## Next measurement

The next optimization tranche should add behavior-preserving instrumentation
for prepare/plan time, per-shard publication and doorbells, wait rounds,
spin/yield/sleep counts, first and last tail readiness, final audit, and
retirement. A counterbalanced 0/25/50/100/200 us wait-policy screen should
measure both CPU cost and tail latency. Queue scheduling should then be
isolated while holding the two physical engines and packets per engine
constant across logical queue counts, with in-phase clocks, power, and engine
counters where the host permits them.

## Claim limits

This qualifies one commit, host, GPU, driver stack, API shape, and striped-copy
workload. E2E is a host-monotonic interval and the wait component is not pure
device execution time. P50 controls the registered gate; p95 is retained but
not gated. Large reference outliers remain in raw evidence and do not control
the medians. The run does not establish causality, physical-link bandwidth,
general copy performance, HIP/HSA parity, or any orders-of-magnitude speedup.
